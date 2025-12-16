use std::collections::VecDeque;
use std::time::{Duration, Instant};

#[cfg(feature = "daemon")]
use sysinfo::{System, RefreshKind, ProcessRefreshKind, CpuRefreshKind, MemoryRefreshKind};

const FRAME_HISTORY_SIZE: usize = 120; // Track last 2 seconds at 60fps

/// Performance statistics tracker
pub struct PerformanceStats {
    /// Frame time history for FPS calculation
    frame_times: VecDeque<Duration>,

    /// Last update time for frame timing
    last_frame: Instant,

    /// System information (CPU, memory)
    #[cfg(feature = "daemon")]
    system: System,

    /// Last time system stats were updated
    last_system_update: Instant,

    /// System update interval (update every 500ms to reduce overhead)
    system_update_interval: Duration,

    /// Cached CPU usage percentage
    cpu_usage: f32,

    /// Cached memory usage in MB
    memory_used_mb: f32,

    /// Cached total memory in MB
    memory_total_mb: f32,

    /// Whether performance overlay is visible
    pub visible: bool,
}

impl PerformanceStats {
    pub fn new() -> Self {
        #[cfg(feature = "daemon")]
        let system = System::new_with_specifics(
            RefreshKind::new()
                .with_cpu(CpuRefreshKind::everything())
                .with_memory(MemoryRefreshKind::everything())
        );

        Self {
            frame_times: VecDeque::with_capacity(FRAME_HISTORY_SIZE),
            last_frame: Instant::now(),
            #[cfg(feature = "daemon")]
            system,
            last_system_update: Instant::now(),
            system_update_interval: Duration::from_millis(500),
            cpu_usage: 0.0,
            memory_used_mb: 0.0,
            memory_total_mb: 0.0,
            visible: false,
        }
    }

    /// Record a new frame and update frame time statistics
    pub fn record_frame(&mut self) {
        let now = Instant::now();
        let frame_time = now.duration_since(self.last_frame);
        self.last_frame = now;

        // Add to history
        self.frame_times.push_back(frame_time);

        // Keep history size limited
        if self.frame_times.len() > FRAME_HISTORY_SIZE {
            self.frame_times.pop_front();
        }

        // Update system stats if enough time has passed
        if now.duration_since(self.last_system_update) >= self.system_update_interval {
            self.update_system_stats();
            self.last_system_update = now;
        }
    }

    /// Update system statistics (CPU, memory)
    #[cfg(feature = "daemon")]
    fn update_system_stats(&mut self) {
        // Refresh CPU and memory
        self.system.refresh_cpu_all();
        self.system.refresh_memory();

        // Calculate average CPU usage across all cores
        let cpus = self.system.cpus();
        if !cpus.is_empty() {
            let total: f32 = cpus.iter().map(|cpu| cpu.cpu_usage()).sum();
            self.cpu_usage = total / cpus.len() as f32;
        }

        // Get memory usage
        self.memory_used_mb = self.system.used_memory() as f32 / 1024.0 / 1024.0;
        self.memory_total_mb = self.system.total_memory() as f32 / 1024.0 / 1024.0;
    }

    #[cfg(not(feature = "daemon"))]
    fn update_system_stats(&mut self) {
        // No-op when daemon feature is not enabled
    }

    /// Get current FPS
    pub fn fps(&self) -> f32 {
        if self.frame_times.is_empty() {
            return 0.0;
        }

        // Calculate average frame time
        let total_time: Duration = self.frame_times.iter().sum();
        let avg_frame_time = total_time.as_secs_f32() / self.frame_times.len() as f32;

        if avg_frame_time > 0.0 {
            1.0 / avg_frame_time
        } else {
            0.0
        }
    }

    /// Get average frame time in milliseconds
    pub fn avg_frame_time_ms(&self) -> f32 {
        if self.frame_times.is_empty() {
            return 0.0;
        }

        let total_time: Duration = self.frame_times.iter().sum();
        let avg = total_time.as_secs_f32() / self.frame_times.len() as f32;
        avg * 1000.0
    }

    /// Get minimum frame time in milliseconds (best frame)
    pub fn min_frame_time_ms(&self) -> f32 {
        self.frame_times
            .iter()
            .min()
            .map(|d| d.as_secs_f32() * 1000.0)
            .unwrap_or(0.0)
    }

    /// Get maximum frame time in milliseconds (worst frame)
    pub fn max_frame_time_ms(&self) -> f32 {
        self.frame_times
            .iter()
            .max()
            .map(|d| d.as_secs_f32() * 1000.0)
            .unwrap_or(0.0)
    }

    /// Get CPU usage percentage (0-100)
    pub fn cpu_usage(&self) -> f32 {
        self.cpu_usage
    }

    /// Get memory usage in MB
    pub fn memory_used_mb(&self) -> f32 {
        self.memory_used_mb
    }

    /// Get total memory in MB
    pub fn memory_total_mb(&self) -> f32 {
        self.memory_total_mb
    }

    /// Get memory usage percentage (0-100)
    pub fn memory_usage_percent(&self) -> f32 {
        if self.memory_total_mb > 0.0 {
            (self.memory_used_mb / self.memory_total_mb) * 100.0
        } else {
            0.0
        }
    }

    /// Toggle visibility of performance overlay
    pub fn toggle_visibility(&mut self) {
        self.visible = !self.visible;
        println!("[Performance] Overlay visibility: {}", self.visible);
    }

    /// Check if performance overlay is visible
    pub fn is_visible(&self) -> bool {
        self.visible
    }
}

impl Default for PerformanceStats {
    fn default() -> Self {
        Self::new()
    }
}
