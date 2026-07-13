use anyhow::{Context, Result};
use std::collections::HashMap;
use std::io::Read;
use std::os::unix::net::UnixDatagram;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::local_definitions::LocalDefinitions;

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
/// The socket file must already exist (Dolphin creates it on startup).
pub fn listen_memorywatcher(cache: MemoryCache) -> Result<()> {
    let socket_path = socket_file_path()?;

    // Remove stale socket if present (Dolphin will recreate it)
    // Actually, Dolphin creates this socket — we just bind to it as a client.
    // MemoryWatcher uses SOCK_DGRAM and sends TO this socket path.
    // We need to bind to the path to receive datagrams.

    // Clean up any stale socket file
    if socket_path.exists() {
        // Check if it's a socket
        let metadata = std::fs::metadata(&socket_path)?;
        use std::os::unix::fs::FileTypeExt;
        if metadata.file_type().is_socket() {
            // Try to remove it — if Dolphin is running, this is its socket
            // and we shouldn't remove it. If stale, we can remove it.
            // Actually, Dolphin sends TO this path, it doesn't bind to it.
            // We bind to it. So we should remove a stale one.
            let _ = std::fs::remove_file(&socket_path);
        }
    }

    let socket = UnixDatagram::bind(&socket_path)
        .with_context(|| format!("Failed to bind MemoryWatcher socket at {:?}", socket_path))?;

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
    /// 2. A tick thread that evaluates conditions at ~60Hz
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

        // Spawn evaluation tick loop
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

            // Track which achievements have already been unlocked this session
            // to avoid duplicate notifications
            let mut unlocked_this_session: std::collections::HashSet<u32> =
                match cache_db.get_local_unlock_ids(&game_hash) {
                    Ok(ids) => ids,
                    Err(_) => std::collections::HashSet::new(),
                };

            loop {
                let is_running = {
                    let mut r = running.lock().unwrap();
                    *r
                };
                if !is_running {
                    break;
                }

                // Evaluate each achievement
                // Note: full rcheevos integration requires the C library.
                // For now, we do a simple evaluation: check if any memory
                // value matches expected conditions. The real implementation
                // will call rc_runtime_tick() via FFI.
                for ach in &defs.achievements {
                    if unlocked_this_session.contains(&ach.id) {
                        continue;
                    }

                    // Check if already unlocked in DB
                    if let Ok(true) = cache_db.is_local_unlocked(ach.id, &game_hash) {
                        unlocked_this_session.insert(ach.id);
                        continue;
                    }

                    // TODO: When rcheevos FFI is integrated, this becomes:
                    //   rc_runtime_tick(runtime, frame_number, &read_memory_callback, ...)
                    // For now, the evaluation is a placeholder. The real
                    // condition evaluation will be done by rcheevos' C engine.
                    let triggered = evaluate_condition_placeholder(&ach.mem_addr, &cache_tick);

                    if triggered {
                        let _ = cache_db.local_unlock(ach.id, &game_hash, false);
                        unlocked_this_session.insert(ach.id);
                        callback(ach.id, &ach.title);
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

/// Placeholder condition evaluator.
///
/// This is a simplified evaluator that handles basic equality checks.
/// The real implementation will use rcheevos' rc_runtime_t API via FFI
/// to evaluate the full condition string grammar (hit counts, deltas,
/// pause conditions, AddSource/SubSource, etc.).
///
/// Supported (simplified):
///   0xH<addr>=<value>  — 8-bit value equals constant
///   0xH<addr>><value>  — 8-bit value greater than constant
///
/// Everything else returns false (not triggered).
fn evaluate_condition_placeholder(condition: &str, cache: &MemoryCache) -> bool {
    // Parse the first condition group (before any dot separator)
    let first_cond = condition.split('.').next().unwrap_or("");

    // Try to parse: [d]0x<size><addr><op><value>
    // where size is H, X, space, L, U, or M
    let bytes = first_cond.as_bytes();
    let mut i = 0;

    // Skip optional 'd' prefix (delta — we don't support this in placeholder)
    if i < bytes.len() && bytes[i] == b'd' {
        return false;
    }

    // Expect "0x"
    if i + 2 >= bytes.len() || bytes[i] != b'0' || bytes[i + 1] != b'x' {
        return false;
    }
    i += 2;

    // Parse size modifier
    let size = if i < bytes.len() {
        match bytes[i] {
            b'H' => {
                i += 1;
                MemorySize::Bit8
            }
            b'X' => {
                i += 1;
                MemorySize::Bit32
            }
            b'L' => {
                i += 1;
                MemorySize::Lower16
            }
            b'U' => {
                i += 1;
                MemorySize::Upper16
            }
            b' ' => {
                i += 1;
                MemorySize::Bit16
            }
            b'M' => {
                i += 1;
                MemorySize::Bit0
            }
            _ => return false,
        }
    } else {
        return false;
    };

    // Parse hex address
    let addr_start = i;
    while i < bytes.len() && bytes[i].is_ascii_hexdigit() {
        i += 1;
    }
    if i == addr_start {
        return false;
    }
    let addr_str = &first_cond[addr_start..i];
    let addr = match u32::from_str_radix(addr_str, 16) {
        Ok(a) => a,
        Err(_) => return false,
    };

    // Parse operator
    if i >= bytes.len() {
        return false;
    }
    let op = bytes[i];
    i += 1;

    // Parse value (hex)
    let val_start = i;
    while i < bytes.len() && bytes[i].is_ascii_hexdigit() {
        i += 1;
    }
    if i == val_start {
        return false;
    }
    let val_str = &first_cond[val_start..i];
    let expected = match u32::from_str_radix(val_str, 16) {
        Ok(v) => v,
        Err(_) => return false,
    };

    let actual = read_memory_value(cache, addr, size);

    match op {
        b'=' => actual == expected,
        b'>' => actual > expected,
        b'<' => actual < expected,
        _ => false,
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
    fn test_evaluate_placeholder_equality() {
        let cache = new_memory_cache();
        {
            let mut c = cache.lock().unwrap();
            c.insert(0x00801234, 0x00000001);
        }
        assert!(evaluate_condition_placeholder("0xH00801234=1", &cache));
        assert!(!evaluate_condition_placeholder("0xH00801234=2", &cache));
    }

    #[test]
    fn test_evaluate_placeholder_greater() {
        let cache = new_memory_cache();
        {
            let mut c = cache.lock().unwrap();
            c.insert(0x00801234, 0x00000005);
        }
        assert!(evaluate_condition_placeholder("0xH00801234>3", &cache));
        assert!(!evaluate_condition_placeholder("0xH00801234>5", &cache));
    }

    #[test]
    fn test_evaluate_placeholder_delta_returns_false() {
        let cache = new_memory_cache();
        // Delta conditions are not supported in the placeholder
        assert!(!evaluate_condition_placeholder("d0xH00801234=1", &cache));
    }
}
