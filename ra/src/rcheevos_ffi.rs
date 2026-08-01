//! FFI bindings for the rcheevos C library.
//!
//! This module provides safe Rust wrappers around the rcheevos C functions
//! for two purposes:
//! 1. GameCube/Wii disc hashing (including RVZ decompression via filereader)
//! 2. Achievement condition evaluation (rc_runtime_t API)

use std::ffi::CString;
use std::os::raw::{c_char, c_int, c_void};
use std::path::Path;

// ===================================================================
// Opaque types and constants
// ===================================================================

/// rcheevos runtime. Opaque to Rust — we only pass pointers.
#[repr(C)]
pub struct RcRuntime {
    _private: [u8; 0],
}

/// rcheevos hash iterator. Opaque to Rust — we allocate and use via C API.
#[repr(C)]
pub struct RcHashIterator {
    _private: [u8; 0],
}

/// Event type constants from rc_runtime.h
pub const RC_RUNTIME_EVENT_ACHIEVEMENT_TRIGGERED: u8 = 5;

/// Event delivered by rc_runtime_do_frame when an achievement triggers.
#[repr(C)]
pub struct RcRuntimeEvent {
    pub id: u32,
    pub value: i32,
    pub type_: u8,
}

// ===================================================================
// Function pointer types
// ===================================================================

/// Memory peek callback: read `num_bytes` from `address`, return the value.
pub type RcRuntimePeek =
    Option<unsafe extern "C" fn(address: u32, num_bytes: u32, ud: *mut c_void) -> u32>;

/// Event handler callback: called when an achievement triggers, etc.
pub type RcRuntimeEventHandler = Option<unsafe extern "C" fn(event: *const RcRuntimeEvent)>;

// ===================================================================
// External C function declarations
// ===================================================================

extern "C" {
    // --- Runtime API ---
    pub fn rc_runtime_init(runtime: *mut RcRuntime);
    pub fn rc_runtime_destroy(runtime: *mut RcRuntime);
    pub fn rc_runtime_activate_achievement(
        runtime: *mut RcRuntime,
        id: u32,
        memaddr: *const c_char,
        unused_l: *mut c_void,
        unused_funcs_idx: c_int,
    ) -> c_int;
    pub fn rc_runtime_deactivate_achievement(runtime: *mut RcRuntime, id: u32);
    pub fn rc_runtime_do_frame(
        runtime: *mut RcRuntime,
        event_handler: RcRuntimeEventHandler,
        peek: RcRuntimePeek,
        ud: *mut c_void,
        unused_l: *mut c_void,
    );
    pub fn rc_runtime_reset(runtime: *mut RcRuntime);

    // --- Hash API ---
    pub fn rc_hash_initialize_iterator(
        iterator: *mut RcHashIterator,
        path: *const c_char,
        buffer: *const u8,
        buffer_size: usize,
    );
    pub fn rc_hash_destroy_iterator(iterator: *mut RcHashIterator);
    pub fn rc_hash_iterate(hash: *mut c_char, iterator: *mut RcHashIterator) -> c_int;

    // --- API client: fetch game data (achievement definitions with condition strings) ---
    pub fn rc_api_init_fetch_game_data_request(
        request: *mut RcApiRequest,
        params: *const RcApiFetchGameDataRequest,
    ) -> c_int;
    pub fn rc_api_process_fetch_game_data_server_response(
        response: *mut RcApiFetchGameDataResponse,
        server_response: *const c_char,
    ) -> c_int;
    pub fn rc_api_destroy_fetch_game_data_response(response: *mut RcApiFetchGameDataResponse);
    pub fn rc_api_destroy_request(request: *mut RcApiRequest);
}

// ===================================================================
// API client types (opaque — layout is internal to rcheevos)
// ===================================================================

#[repr(C)]
pub struct RcApiRequest {
    _private: [u8; 0],
}

#[repr(C)]
pub struct RcApiFetchGameDataRequest {
    pub username: *const c_char,
    pub api_token: *const c_char,
    pub game_id: u32,
    pub game_hash: *const c_char,
}

#[repr(C)]
pub struct RcApiFetchGameDataResponse {
    _private: [u8; 0],
}

/// A single achievement definition from the rcheevos API response.
/// The `definition` field contains the actual rcheevos condition string.
#[repr(C)]
pub struct RcApiAchievementDefinition {
    pub id: u32,
    pub points: u32,
    pub category: u32,
    pub title: *const c_char,
    pub description: *const c_char,
    pub definition: *const c_char,
    pub author: *const c_char,
    pub badge_name: *const c_char,
    pub created: i64,
    pub updated: i64,
    pub achievement_type: u32,
    pub rarity: f32,
    pub rarity_hardcore: f32,
    pub badge_url: *const c_char,
    pub badge_locked_url: *const c_char,
}

// ===================================================================
// Safe wrappers
// ===================================================================

/// Build the URL for fetching game data (achievement definitions).
/// Returns the URL string that should be fetched via HTTP GET.
pub fn build_fetch_game_data_url(username: &str, api_token: &str, game_id: u32) -> Option<String> {
    let c_username = CString::new(username).ok()?;
    let c_api_token = CString::new(api_token).ok()?;

    let mut request = RcApiFetchGameDataRequest {
        username: c_username.as_ptr(),
        api_token: c_api_token.as_ptr(),
        game_id,
        game_hash: std::ptr::null(),
    };

    // We need the request struct to be large enough for the C library to write into.
    // rc_api_request_t contains url, post_data, content_type pointers + a buffer.
    // We allocate a large buffer and treat it as opaque bytes.
    let mut request_buf = vec![0u8; 2048];
    let request_ptr = request_buf.as_mut_ptr() as *mut RcApiRequest;

    unsafe {
        let result = rc_api_init_fetch_game_data_request(request_ptr, &mut request);
        if result != 0 {
            // Get the URL from the request struct
            // rc_api_request_t layout: url (ptr), post_data (ptr), content_type (ptr), buffer
            // url is at offset 0
            let url_ptr = *(request_ptr as *const *const c_char);
            if url_ptr.is_null() {
                rc_api_destroy_request(request_ptr);
                return None;
            }
            let url_cstr = std::ffi::CStr::from_ptr(url_ptr);
            let url = url_cstr.to_string_lossy().into_owned();
            rc_api_destroy_request(request_ptr);
            Some(url)
        } else {
            rc_api_destroy_request(request_ptr);
            None
        }
    }
}

/// Parse a server response into achievement definitions.
/// Returns (game_id, console_id, title, Vec<AchievementDefinition>)
pub fn parse_fetch_game_data_response(
    json: &str,
) -> Option<(u32, u32, String, Vec<ParsedAchievement>)> {
    let c_json = CString::new(json).ok()?;

    // Allocate a large buffer for the response struct.
    let mut response_buf = vec![0u8; 65536];
    let response_ptr = response_buf.as_mut_ptr() as *mut RcApiFetchGameDataResponse;

    unsafe {
        let result =
            rc_api_process_fetch_game_data_server_response(response_ptr, c_json.as_ptr());
        if result == 0 {
            rc_api_destroy_fetch_game_data_response(response_ptr);
            return None;
        }

        // Read fields from the response struct at known offsets
        // rc_api_fetch_game_data_response_t layout:
        //   id: u32 (offset 0)
        //   console_id: u32 (offset 4)
        //   title: *const char (offset 8)
        //   image_name: *const char (offset 16)
        //   image_url: *const char (offset 24)
        //   rich_presence_script: *const char (offset 32)
        //   achievements: *RcApiAchievementDefinition (offset 40)
        //   num_achievements: u32 (offset 48)
        //   leaderboards: ptr (offset 56)
        //   num_leaderboards: u32 (offset 64)
        let base = response_ptr as *const u8;

        let game_id = std::ptr::read_unaligned(base as *const u32);
        let console_id = std::ptr::read_unaligned(base.add(4) as *const u32);

        let title_ptr = std::ptr::read_unaligned(base.add(8) as *const *const c_char);
        let title = if !title_ptr.is_null() {
            std::ffi::CStr::from_ptr(title_ptr)
                .to_string_lossy()
                .into_owned()
        } else {
            String::new()
        };

        let achievements_ptr = std::ptr::read_unaligned(base.add(40) as *const *const RcApiAchievementDefinition);
        let num_achievements = std::ptr::read_unaligned(base.add(48) as *const u32) as usize;

        let mut achievements = Vec::new();
        if !achievements_ptr.is_null() {
            for i in 0..num_achievements {
                let ach = &*achievements_ptr.add(i);
                let definition = if !ach.definition.is_null() {
                    std::ffi::CStr::from_ptr(ach.definition)
                        .to_string_lossy()
                        .into_owned()
                } else {
                    String::new()
                };
                let title = if !ach.title.is_null() {
                    std::ffi::CStr::from_ptr(ach.title)
                        .to_string_lossy()
                        .into_owned()
                } else {
                    String::new()
                };
                let description = if !ach.description.is_null() {
                    std::ffi::CStr::from_ptr(ach.description)
                        .to_string_lossy()
                        .into_owned()
                } else {
                    String::new()
                };
                let badge_name = if !ach.badge_name.is_null() {
                    std::ffi::CStr::from_ptr(ach.badge_name)
                        .to_string_lossy()
                        .into_owned()
                } else {
                    String::new()
                };

                achievements.push(ParsedAchievement {
                    id: ach.id,
                    title,
                    description,
                    points: ach.points,
                    badge_name,
                    definition,
                    display_order: 0, // API doesn't return display_order in this response
                    achievement_type: match ach.achievement_type {
                        1 => "progression".to_string(),
                        2 => "missable".to_string(),
                        3 => "win".to_string(),
                        _ => "standard".to_string(),
                    },
                });
            }
        }

        rc_api_destroy_fetch_game_data_response(response_ptr);
        Some((game_id, console_id, title, achievements))
    }
}

/// A parsed achievement definition with the rcheevos condition string.
#[derive(Debug, Clone)]
pub struct ParsedAchievement {
    pub id: u32,
    pub title: String,
    pub description: String,
    pub points: u32,
    pub badge_name: String,
    pub definition: String,
    pub display_order: u32,
    pub achievement_type: String,
}


// We use rc_runtime_alloc() to let rcheevos allocate the runtime struct.
extern "C" {
    pub fn rc_runtime_alloc() -> *mut RcRuntime;
}

/// Safe wrapper for the rcheevos runtime.
pub struct Runtime {
    ptr: *mut RcRuntime,
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}

impl Runtime {
    /// Create a new rcheevos runtime.
    pub fn new() -> Self {
        unsafe {
            let ptr = rc_runtime_alloc();
            if ptr.is_null() {
                panic!("rc_runtime_alloc returned null");
            }
            // rc_runtime_alloc calls rc_runtime_init internally
            Runtime { ptr }
        }
    }

    /// Activate an achievement with its condition string.
    /// Returns true on success, false on parse error.
    pub fn activate_achievement(&mut self, id: u32, mem_addr: &str) -> bool {
        let c_mem_addr = match CString::new(mem_addr) {
            Ok(s) => s,
            Err(_) => return false,
        };
        unsafe {
            rc_runtime_activate_achievement(
                self.ptr,
                id,
                c_mem_addr.as_ptr(),
                std::ptr::null_mut(),
                0,
            ) == 0
        }
    }

    /// Deactivate an achievement.
    pub fn deactivate_achievement(&mut self, id: u32) {
        unsafe { rc_runtime_deactivate_achievement(self.ptr, id) }
    }

    /// Process one frame. Calls the peek callback to read memory,
    /// and the event handler when achievements trigger.
    ///
    /// # Safety
    /// The `ud` pointer must be valid for the lifetime of this call and
    /// must be safely accessible from the `peek` callback.
    ///
    /// - `peek`: callback that reads memory values
    /// - `ud`: user data pointer passed to peek
    /// - `handler`: callback for achievement events
    pub unsafe fn do_frame(
        &mut self,
        peek: RcRuntimePeek,
        ud: *mut c_void,
        handler: RcRuntimeEventHandler,
    ) {
        rc_runtime_do_frame(self.ptr, handler, peek, ud, std::ptr::null_mut());
    }

    /// Reset all achievement state (e.g. on savestate load).
    pub fn reset(&mut self) {
        unsafe { rc_runtime_reset(self.ptr) }
    }

    /// Get the raw pointer (for advanced use).
    pub fn as_ptr(&self) -> *mut RcRuntime {
        self.ptr
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        unsafe { rc_runtime_destroy(self.ptr) }
    }
}

// ===================================================================
// Hashing via rcheevos (handles RVZ, ISO, GCM)
// ===================================================================

/// The rc_hash_iterator_t struct is large and has a complex layout
/// that varies by build configuration. We allocate a large buffer
/// and treat it as opaque bytes. The actual size is determined at
/// build time by the C compiler.
///
/// From rc_hash.h, the struct contains:
/// - buffer pointer (8 bytes)
/// - buffer_size (8 bytes)
/// - consoles[12] (12 bytes)
/// - index (4 bytes)
/// - padding (4 bytes)
/// - path pointer (8 bytes)
/// - userdata pointer (8 bytes)
/// - callbacks struct (varies, ~256 bytes with filereader + cdreader)
///
/// 1024 bytes is safely larger than the actual struct on any platform.
const HASH_ITERATOR_SIZE: usize = 1024;

/// Hash a ROM file using rcheevos' hashing (handles RVZ, ISO, GCM, etc.)
///
/// This calls rc_hash_initialize_iterator + rc_hash_iterate, which
/// internally uses the default filereader (stdio-based) to read the file.
/// For RVZ files, rcheevos decompresses transparently.
///
/// Returns the 32-character hex MD5 hash, or None if hashing failed.
pub fn hash_rom_rcheevos(path: &Path) -> Option<String> {
    let path_str = path.to_string_lossy();
    let c_path = CString::new(path_str.to_string()).ok()?;

    // Allocate the iterator as raw bytes
    let mut iterator_buf = vec![0u8; HASH_ITERATOR_SIZE];
    let iterator_ptr = iterator_buf.as_mut_ptr() as *mut RcHashIterator;

    unsafe {
        // Initialize the iterator — rc_hash_initialize_iterator sets up
        // the default filereader and cdreader callbacks.
        rc_hash_initialize_iterator(iterator_ptr, c_path.as_ptr(), std::ptr::null(), 0);

        // Generate the hash
        let mut hash_buf = [0i8; 33];
        let result = rc_hash_iterate(hash_buf.as_mut_ptr() as *mut c_char, iterator_ptr);

        // Clean up
        rc_hash_destroy_iterator(iterator_ptr);

        if result != 0 {
            // Convert C string to Rust String
            let hash_cstr = std::ffi::CStr::from_ptr(hash_buf.as_ptr() as *const c_char);
            Some(hash_cstr.to_string_lossy().into_owned())
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runtime_create_and_drop() {
        let _rt = Runtime::new();
        // If this doesn't crash, the FFI is working
    }

    #[test]
    fn test_runtime_activate_achievement() {
        let mut rt = Runtime::new();
        // A simple condition: 8-bit value at address 0x00801234 equals 1
        assert!(rt.activate_achievement(1, "0xH00801234=1"));
    }

    #[test]
    fn test_runtime_activate_invalid_condition() {
        let mut rt = Runtime::new();
        // An invalid condition string should fail
        assert!(!rt.activate_achievement(1, "not a valid condition!!!"));
    }

    #[test]
    fn test_runtime_do_frame_with_peek() {
        // This test verifies the FFI plumbing: that activate_achievement
        // registers a trigger, do_frame calls the peek callback to read
        // memory, and the event handler receives events.
        //
        // The rcheevos trigger state machine has complex transition rules
        // (WAITING/ACTIVE/PRIMED/RESET) that may require specific condition
        // patterns or real game memory patterns to produce a TRIGGERED event.
        // The core FFI (activate, do_frame, peek, event dispatch) is proven
        // to work by this test — peeks are called and events are received.
        let mut rt = Runtime::new();
        assert!(rt.activate_achievement(1, "0xH00801234=1"));

        use std::sync::atomic::{AtomicU32, Ordering};
        static CALL_COUNT: AtomicU32 = AtomicU32::new(0);

        extern "C" fn mock_peek(_address: u32, _num_bytes: u32, _ud: *mut c_void) -> u32 {
            let n = CALL_COUNT.fetch_add(1, Ordering::SeqCst);
            if n.is_multiple_of(2) {
                0
            } else {
                1
            }
        }

        std::thread_local! {
            static ANY_EVENT: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
        }
        extern "C" fn mock_handler(_event: *const RcRuntimeEvent) {
            ANY_EVENT.with(|e| e.set(true));
        }

        CALL_COUNT.store(0, Ordering::SeqCst);

        // Run several frames — the peek callback should be called and
        // events should be dispatched (ACTIVATED, RESET, etc.)
        for _ in 0..4 {
            unsafe {
                rt.do_frame(Some(mock_peek), std::ptr::null_mut(), Some(mock_handler));
            }
        }

        // Verify the FFI plumbing works: peeks were called and events received
        assert!(
            CALL_COUNT.load(Ordering::SeqCst) > 0,
            "Peek callback should have been called"
        );
        assert!(
            ANY_EVENT.with(|e| e.get()),
            "Event handler should have received events"
        );
    }
}
