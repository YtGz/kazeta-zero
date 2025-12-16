# Performance Issues Quick Reference

## Critical (P1) - Fix First

### 1. ROM Hashing Loads Entire File Into Memory
**Location:** `ra/src/hash.rs:14-16`
**Impact:** N64 ROMs (64MB+) cause memory spikes
**Current Code:**
```rust
let mut buffer = Vec::new();
file.read_to_end(&mut buffer)?;
```
**Fix:** Use streaming hash with buffered reader
```rust
let mut hasher = Md5::new();
let mut reader = BufReader::with_capacity(1024 * 1024, file);
loop {
    let bytes_read = reader.read(&mut chunk)?;
    if bytes_read == 0 { break; }
    hasher.update(&chunk[..bytes_read]);
}
```

### 2. Overlay Wastes CPU When Hidden
**Location:** `overlay/src/main.rs:136-148`
**Impact:** Full 60 FPS loop even when overlay not visible
**Current Code:**
```rust
if overlay_state.should_render() {
    rendering::render(&overlay_state).await;
} else {
    next_frame().await;  // Still runs at 60 FPS
}
```
**Fix:** Reduce loop frequency when hidden
```rust
if !overlay_state.should_render() && !overlay_state.performance.is_visible() {
    std::thread::sleep(Duration::from_millis(50));  // 20 FPS when idle
    next_frame().await;
    continue;
}
```

### 3. Input-Daemon Polls For New Devices
**Location:** `input-daemon/src/main.rs:276-306`
**Impact:** Filesystem scan every 2 seconds
**Current Code:**
```rust
// Scans /dev/input every DEVICE_SCAN_INTERVAL_MS
while running.load(Ordering::Relaxed) {
    let new_devices = find_input_devices(&state);
    // ...
    thread::sleep(Duration::from_millis(100));
}
```
**Fix:** Use inotify for event-driven detection
```rust
use inotify::{Inotify, WatchMask};
let mut inotify = Inotify::init()?;
inotify.watches().add("/dev/input", WatchMask::CREATE | WatchMask::DELETE)?;
// Block on inotify.read_events() instead of polling
```

### 4. Blocking HTTP in RA API
**Location:** `ra/src/api.rs:9`
**Impact:** Blocks main thread during API calls
**Current Code:**
```rust
client: reqwest::blocking::Client,
```
**Fix for library use:** Add async API variant
```rust
pub struct AsyncRAClient {
    client: reqwest::Client,
}

impl AsyncRAClient {
    pub async fn get_game_info(&self, game_id: u32) -> Result<GameInfo> {
        // Non-blocking implementation
    }
}
```

---

## Important (P2) - Fix Soon

### 5. Toast Queue Unbounded Growth
**Location:** `overlay/src/state.rs:712-713`
**Impact:** Memory grows with rapid toast additions
**Fix:** Add max capacity
```rust
const MAX_TOASTS: usize = 10;
pub fn add_toast(&mut self, ...) {
    if self.queue.len() >= MAX_TOASTS {
        self.queue.pop_front();
    }
    self.queue.push_back(toast);
}
```

### 6. Double JSON Parse in API Response
**Location:** `ra/src/api.rs:63-71`
**Impact:** Unnecessary string allocation and parsing
**Current Code:**
```rust
let text = response.text()?;
if text == "{}" || text.is_empty() || text.contains("\"ID\":0") {
    return Ok(None);
}
let lookup: GameInfoAndProgress = serde_json::from_str(&text)?;
```
**Fix:** Parse once with proper null handling
```rust
#[derive(Deserialize)]
struct OptionalGame {
    #[serde(rename = "ID")]
    id: Option<u32>,
}
let result: OptionalGame = response.json()?;
match result.id {
    Some(id) if id != 0 => Ok(Some(id)),
    _ => Ok(None),
}
```

### 7. String Allocations in Render Loop
**Location:** `overlay/src/rendering.rs` (multiple)
**Impact:** GC pressure from format!() calls every frame
**Examples:**
```rust
// Line 180 - allocates every frame
let progress_text = format!("{}/{} ({:.0}%)", earned, total, progress * 100.0);
// Line 306
let status_text = format!("{} controller(s) connected", controller_count);
```
**Fix:** Pre-allocate reusable strings or use write!()
```rust
// In state, pre-allocate:
pub progress_text_buffer: String,

// In update:
self.progress_text_buffer.clear();
write!(&mut self.progress_text_buffer, "{}/{}", earned, total).unwrap();
```

### 8. System Stats Both Updated Together
**Location:** `overlay/src/performance.rs:86-89`
**Impact:** CPU spike every 500ms when both refresh
**Current Code:**
```rust
self.system.refresh_cpu_all();
self.system.refresh_memory();
```
**Fix:** Stagger updates
```rust
let now = Instant::now();
if now.duration_since(self.last_cpu_update) >= Duration::from_millis(500) {
    self.system.refresh_cpu_all();
    self.last_cpu_update = now;
} else if now.duration_since(self.last_mem_update) >= Duration::from_millis(500) {
    self.system.refresh_memory();
    self.last_mem_update = now;
}
```

---

## Nice to Have (P3) - Optimize Later

### 9. Frame Timing Precision
**Location:** `overlay/src/main.rs:145-147`
**Impact:** Sleep variance on some systems
**Note:** Only matters for frame-sensitive applications

### 10. Per-Device Thread Model
**Location:** `input-daemon/src/main.rs:190-273`
**Impact:** Thread count grows with input devices
**Note:** Works fine with typical 2-4 devices

### 11. Achievement Texture Loading
**Location:** `overlay/src/state.rs`
**Impact:** Achievement images not loaded (text only)
**Note:** Feature request, not performance issue

---

## Measurement Commands

### Profile Overlay CPU Usage
```bash
# Run overlay with perf
perf record -g ./target/release/kazeta-overlay
perf report

# Or with flamegraph
cargo flamegraph --bin kazeta-overlay
```

### Monitor Input-Daemon Threads
```bash
# Count threads
ps -T -p $(pgrep kazeta-input) | wc -l

# Watch for thread changes
watch -n1 "ps -T -p \$(pgrep kazeta-input)"
```

### Profile RA Hashing
```bash
# Time large ROM hashing
time kazeta-ra hash-rom --path /path/to/large.n64 --console n64

# Memory usage during hash
/usr/bin/time -v kazeta-ra hash-rom --path /path/to/large.n64 --console n64
```

### Benchmark IPC Throughput
```bash
# Send many messages quickly
for i in {1..100}; do
  echo '{"type":"show_toast","message":"Test '$i'","style":"info","duration_ms":100}' | \
    nc -U /tmp/kazeta-overlay.sock
done
```
