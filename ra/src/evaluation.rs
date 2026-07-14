use anyhow::{Context, Result};
use std::collections::HashMap;
use std::os::raw::c_void;
use std::os::unix::net::UnixDatagram;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::local_definitions::LocalDefinitions;
use crate::rcheevos_ffi::{
    RcRuntimeEvent, Runtime as RcRuntime, RC_RUNTIME_EVENT_ACHIEVEMENT_TRIGGERED,
};

/// Default Dolphin user directory paths for MemoryWatcher.
/// Dolphin looks for these in its user directory (typically
/// ~/.local/share/dolphin-emu/ on Linux).
const MEMORYWATCHER_DIR: &str = "MemoryWatcher";
const LOCATIONS_FILE: &str = "Locations.txt";
const SOCKET_FILE: &str = "MemoryWatcher";

/// Find the Dolphin user directory for MemoryWatcher files.
///
/// Checks the standard location (~/.local/share/dolphin-emu/) and
/// the portable location (./User/ next to the Dolphin binary).
pub fn find_dolphin_user_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().context("Could not find home directory")?;

    let candidate = home.join(".local/share/dolphin-emu");
    if candidate.exists() {
        return Ok(candidate);
    }

    // Portable mode: ./User/ next to the executable
    let portable = PathBuf::from("./User");
    if portable.exists() {
        return Ok(portable);
    }

    // Return the standard path even if it doesn't exist yet —
    // we'll create it when writing Locations.txt
    Ok(candidate)
}

/// Get the path to the MemoryWatcher directory.
pub fn memorywatcher_dir() -> Result<PathBuf> {
    Ok(find_dolphin_user_dir()?.join(MEMORYWATCHER_DIR))
}

/// Get the path to the Locations.txt file.
pub fn locations_file_path() -> Result<PathBuf> {
    Ok(memorywatcher_dir()?.join(LOCATIONS_FILE))
}

/// Get the path to the MemoryWatcher socket.
pub fn socket_file_path() -> Result<PathBuf> {
    Ok(memorywatcher_dir()?.join(SOCKET_FILE))
}

/// Write the memory addresses from local definitions to Dolphin's
/// Locations.txt file so MemoryWatcher knows which addresses to watch.
///
/// MemoryWatcher expects hex addresses (without 0x prefix), one per line.
/// Pointer chains are supported (space-separated offsets).
pub fn write_locations_file(defs: &LocalDefinitions) -> Result<()> {
    let dir = memorywatcher_dir()?;
    std::fs::create_dir_all(&dir).context("Failed to create MemoryWatcher directory")?;

    let path = locations_file_path()?;
    let addresses = defs.extract_memory_addresses();
    let addr_count = addresses.len();

    let mut content = String::new();
    for addr in addresses {
        content.push_str(&format!("{:08X}\n", addr));
    }

    std::fs::write(&path, content)
        .with_context(|| format!("Failed to write Locations.txt to {:?}", path))?;

    tracing_debug(&format!("Wrote {} addresses to {:?}", addr_count, path));

    Ok(())
}

/// Memory cache shared between the socket listener and the evaluation loop.
/// Maps aligned 4-byte addresses to their latest u32 values from MemoryWatcher.
pub type MemoryCache = Arc<Mutex<HashMap<u32, u32>>>;

/// Create a new empty memory cache.
pub fn new_memory_cache() -> MemoryCache {
    Arc::new(Mutex::new(HashMap::new()))
}

/// Parse a MemoryWatcher datagram and update the memory cache.
///
/// MemoryWatcher sends datagrams containing lines of:
///   <address_from_locations_txt>\n<new_value_hex>\n
/// for each address whose value changed since the last frame.
///
/// The address in the datagram matches what was written to Locations.txt
/// (the aligned address). The value is a hex string (without 0x).
pub fn parse_memorywatcher_datagram(data: &[u8], cache: &MemoryCache) {
    let text = String::from_utf8_lossy(data);
    let mut lines = text.lines();

    while let Some(addr_line) = lines.next() {
        if let Some(value_line) = lines.next() {
            // Parse the address (hex, as written in Locations.txt)
            if let Ok(addr) = u32::from_str_radix(addr_line.trim(), 16) {
                // Parse the value (hex)
                if let Ok(value) = u32::from_str_radix(value_line.trim(), 16) {
                    let mut cache_lock = cache.lock().unwrap();
                    cache_lock.insert(addr, value);
                }
            }
        }
    }
}

/// Listen for MemoryWatcher datagrams and update the memory cache.
///
/// This blocks the calling thread. It should be spawned in a separate thread.
/// The socket directory must exist — we create it and bind before Dolphin starts.
/// If the socket is already in use (stale from a previous run), we remove it first.
pub fn listen_memorywatcher(cache: MemoryCache) -> Result<()> {
    let socket_path = socket_file_path()?;

    // Ensure the MemoryWatcher directory exists
    if let Some(parent) = socket_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    // Remove stale socket file from a previous run
    if socket_path.exists() {
        let _ = std::fs::remove_file(&socket_path);
    }

    // Bind to the socket path with retries (Dolphin may not have started yet)
    let socket = {
        let mut last_err = None;
        let mut bound_socket: Option<UnixDatagram> = None;
        for attempt in 0..30 {
            match UnixDatagram::bind(&socket_path) {
                Ok(s) => {
                    bound_socket = Some(s);
                    break;
                }
                Err(e) => {
                    last_err = Some(e);
                    if attempt < 29 {
                        std::thread::sleep(Duration::from_millis(500));
                    }
                }
            }
        }
        match bound_socket {
            Some(s) => s,
            None => {
                return Err(anyhow::anyhow!(
                    "Failed to bind MemoryWatcher socket at {:?} after 30 attempts: {}",
                    socket_path,
                    last_err.unwrap()
                ))
            }
        }
    };

    tracing_debug(&format!(
        "Listening for MemoryWatcher datagrams on {:?}",
        socket_path
    ));

    let mut buf = [0u8; 65536];

    loop {
        match socket.recv(&mut buf) {
            Ok(n) => {
                parse_memorywatcher_datagram(&buf[..n], &cache);
            }
            Err(e) => {
                tracing_debug(&format!("MemoryWatcher socket error: {}", e));
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    }
}

/// Read a value from the memory cache at the given address.
///
/// rcheevos conditions reference specific sizes:
///   0xH = 8-bit (low byte of u32)
///   0x  = 16-bit (low 16 bits of u32)
///   0xX = 32-bit (full u32)
///   0xL = lower 16 bits
///   0xU = upper 16 bits
///
/// This function reads the u32 at the aligned address and extracts
/// the requested portion.
pub fn read_memory_value(cache: &MemoryCache, addr: u32, size: MemorySize) -> u32 {
    let aligned = addr & 0xFFFF_FFFC;
    let cache_lock = cache.lock().unwrap();
    let value = cache_lock.get(&aligned).copied().unwrap_or(0);

    match size {
        MemorySize::Bit8 => value & 0xFF,
        MemorySize::Bit16 => value & 0xFFFF,
        MemorySize::Bit32 => value,
        MemorySize::Lower16 => value & 0xFFFF,
        MemorySize::Upper16 => (value >> 16) & 0xFFFF,
        MemorySize::Bit0 => (value >> (addr & 7)) & 1,
    }
}

/// Memory size specifier for rcheevos condition references.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemorySize {
    /// 0xH — 8-bit value
    Bit8,
    /// 0x (space) — 16-bit value
    Bit16,
    /// 0xX — 32-bit value
    Bit32,
    /// 0xL — lower 16 bits
    Lower16,
    /// 0xU — upper 16 bits
    Upper16,
    /// 0xM / 0xT — bit N (address & 7 gives bit index)
    Bit0,
}

/// The evaluation engine: manages memory cache, socket listener,
/// and a tick loop that evaluates achievement conditions.
pub struct EvaluationEngine {
    cache: MemoryCache,
    defs: LocalDefinitions,
    game_hash: String,
    running: Arc<Mutex<bool>>,
}

impl EvaluationEngine {
    /// Create a new evaluation engine for the given game.
    pub fn new(defs: LocalDefinitions, game_hash: String) -> Result<Self> {
        // Write the Locations.txt file so Dolphin watches the right addresses
        write_locations_file(&defs)?;

        let cache = new_memory_cache();

        Ok(Self {
            cache,
            defs,
            game_hash,
            running: Arc::new(Mutex::new(false)),
        })
    }

    /// Start the evaluation engine in background threads.
    ///
    /// Spawns:
    /// 1. A socket listener thread that receives MemoryWatcher datagrams
    /// 2. A tick thread that calls rcheevos' rc_runtime_do_frame at ~60Hz
    ///
    /// Returns a handle that can be used to stop the engine.
    pub fn start(&self, unlock_callback: impl Fn(u32, &str) + Send + Sync + 'static) -> Result<()> {
        {
            let mut running = self.running.lock().unwrap();
            *running = true;
        }

        // Spawn socket listener
        let cache_listener = self.cache.clone();
        std::thread::spawn(move || {
            let _ = listen_memorywatcher(cache_listener);
        });

        // Spawn evaluation tick loop using rcheevos
        let cache_tick = self.cache.clone();
        let defs = self.defs.clone();
        let game_hash = self.game_hash.clone();
        let running = self.running.clone();
        let callback = Arc::new(unlock_callback);

        std::thread::spawn(move || {
            let tick_interval = Duration::from_millis(1000 / 60);
            let cache_db = match crate::cache::RACache::new() {
                Ok(c) => c,
                Err(e) => {
                    tracing_debug(&format!("Failed to open cache: {}", e));
                    return;
                }
            };

            // Initialize rcheevos runtime and activate all achievements
            let mut runtime = RcRuntime::new();
            let mut active_count = 0u32;
            for ach in &defs.achievements {
                if cache_db
                    .is_local_unlocked(ach.id, &game_hash)
                    .unwrap_or(false)
                {
                    continue; // Already unlocked, don't activate
                }
                if runtime.activate_achievement(ach.id, &ach.mem_addr) {
                    active_count += 1;
                } else {
                    tracing_debug(&format!(
                        "Failed to activate achievement #{}: {}",
                        ach.id, ach.mem_addr
                    ));
                }
            }
            tracing_debug(&format!(
                "Activated {} of {} achievements",
                active_count,
                defs.achievements.len()
            ));

            // Track unlocked IDs to avoid duplicate notifications
            let unlocked_ids: Arc<Mutex<std::collections::HashSet<u32>>> = Arc::new(Mutex::new(
                cache_db
                    .get_local_unlock_ids(&game_hash)
                    .unwrap_or_default(),
            ));

            // The peek callback reads from our MemoryWatcher cache.
            // rcheevos calls this for each memory address it needs.
            let cache_for_peek = cache_tick.clone();

            extern "C" fn peek_callback(address: u32, num_bytes: u32, ud: *mut c_void) -> u32 {
                let cache = unsafe { &*(ud as *const MemoryCache) };
                let aligned = address & 0xFFFF_FFFC;
                let cache_lock = cache.lock().unwrap();
                let value = cache_lock.get(&aligned).copied().unwrap_or(0);

                // rcheevos expects values in little-endian byte order.
                // MemoryWatcher sends big-endian hex values which we stored as u32.
                // For 1-byte reads, return the byte at the correct offset.
                // For 2-byte reads, return the correct half.
                // For 4-byte reads, return the full value.
                // GameCube is big-endian, so the u32 we got is already in
                // big-endian order. rcheevos reads memory as raw bytes,
                // so we return the value as-is for 4-byte reads.
                match num_bytes {
                    1 => {
                        let byte_offset = address & 0x03;
                        (value >> ((3 - byte_offset) * 8)) & 0xFF
                    }
                    2 => {
                        let half_offset = (address & 0x02) >> 1;
                        (value >> ((1 - half_offset) * 16)) & 0xFFFF
                    }
                    _ => value,
                }
            }

            // The event handler is called by rcheevos when achievements trigger.
            // We use a thread-local to communicate the triggered ID back to the
            // tick loop. This is safe because event_handler is called
            // synchronously from do_frame on the same thread.
            let unlocked_for_handler = unlocked_ids.clone();
            let callback_for_handler = callback.clone();
            let game_hash_for_handler = game_hash.clone();

            std::thread_local! {
                static TRIGGERED_ACHIEVEMENT: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
            }

            extern "C" fn event_handler(event: *const RcRuntimeEvent) {
                unsafe {
                    if (*event).type_ == RC_RUNTIME_EVENT_ACHIEVEMENT_TRIGGERED {
                        let id = (*event).id;
                        TRIGGERED_ACHIEVEMENT.with(|cell| cell.set(id));
                    }
                }
            }

            loop {
                let is_running = {
                    let r = running.lock().unwrap();
                    *r
                };
                if !is_running {
                    break;
                }

                // Reset the triggered ID before each frame
                TRIGGERED_ACHIEVEMENT.with(|cell| cell.set(0));

                // Process one frame through rcheevos
                let cache_ptr = &cache_for_peek as *const MemoryCache as *mut c_void;
                runtime.do_frame(Some(peek_callback), cache_ptr, Some(event_handler));

                // Check if an achievement was triggered this frame
                let triggered_id = TRIGGERED_ACHIEVEMENT.with(|cell| cell.get());
                if triggered_id != 0 {
                    let mut unlocked = unlocked_for_handler.lock().unwrap();
                    if !unlocked.contains(&triggered_id) {
                        unlocked.insert(triggered_id);
                        drop(unlocked);

                        // Find the achievement title
                        if let Some(ach) = defs.achievements.iter().find(|a| a.id == triggered_id) {
                            let _ =
                                cache_db.local_unlock(triggered_id, &game_hash_for_handler, false);
                            callback_for_handler(triggered_id, &ach.title);
                            tracing_debug(&format!(
                                "Achievement unlocked: #{} {}",
                                triggered_id, ach.title
                            ));
                        }
                    }
                }

                std::thread::sleep(tick_interval);
            }
        });

        Ok(())
    }

    /// Stop the evaluation engine.
    pub fn stop(&self) {
        let mut running = self.running.lock().unwrap();
        *running = false;
    }
}

fn tracing_debug(msg: &str) {
    // In production, this would use a proper logging framework.
    // For now, just eprintln to stderr (won't interfere with stdout JSON).
    eprintln!("[kazeta-ra eval] {}", msg);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_memorywatcher_datagram() {
        let cache = new_memory_cache();
        // Simulate a datagram: address 00801234, value 00000001
        let datagram = b"00801234\n00000001\n00805678\n00000002\n";
        parse_memorywatcher_datagram(datagram, &cache);

        let cache_lock = cache.lock().unwrap();
        assert_eq!(*cache_lock.get(&0x00801234).unwrap(), 1);
        assert_eq!(*cache_lock.get(&0x00805678).unwrap(), 2);
    }

    #[test]
    fn test_read_memory_value_8bit() {
        let cache = new_memory_cache();
        {
            let mut c = cache.lock().unwrap();
            c.insert(0x00801234, 0x1234ABCD);
        }
        assert_eq!(
            read_memory_value(&cache, 0x00801234, MemorySize::Bit8),
            0xCD
        );
    }

    #[test]
    fn test_read_memory_value_16bit() {
        let cache = new_memory_cache();
        {
            let mut c = cache.lock().unwrap();
            c.insert(0x00801234, 0x1234ABCD);
        }
        assert_eq!(
            read_memory_value(&cache, 0x00801234, MemorySize::Bit16),
            0xABCD
        );
    }

    #[test]
    fn test_read_memory_value_32bit() {
        let cache = new_memory_cache();
        {
            let mut c = cache.lock().unwrap();
            c.insert(0x00801234, 0x1234ABCD);
        }
        assert_eq!(
            read_memory_value(&cache, 0x00801234, MemorySize::Bit32),
            0x1234ABCD
        );
    }

    #[test]
    fn test_read_memory_value_upper16() {
        let cache = new_memory_cache();
        {
            let mut c = cache.lock().unwrap();
            c.insert(0x00801234, 0x1234ABCD);
        }
        assert_eq!(
            read_memory_value(&cache, 0x00801234, MemorySize::Upper16),
            0x1234
        );
    }
}
