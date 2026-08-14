// What this process is costing the machine
//
// Reading your own process needs no entitlement on macOS, unlike inspecting
// somebody else's, so this is a plain sysinfo lookup against our own pid.

use serde::Serialize;
use std::sync::Mutex;
use sysinfo::{get_current_pid, Pid, ProcessRefreshKind, System};

/// A reading of this process's own resource use
#[derive(Debug, Clone, Copy, Serialize, Default)]
pub struct ProcessMetrics {
    /// Percentage of a single core
    ///
    /// Summed across cores, so a busy audio thread alongside the UI reads above
    /// 100. This is the same figure Activity Monitor shows, which is the number
    /// worth agreeing with.
    pub cpu_percent: f32,
    /// Resident set size, in bytes
    pub memory_bytes: u64,
    /// False until CPU has been sampled twice, since it is a difference over time
    pub cpu_ready: bool,
}

/// Samples this process on demand
pub struct ProcessMonitor {
    /// Guarded because refreshing mutates, and readings are wanted from anywhere
    system: Mutex<System>,
    pid: Option<Pid>,
    /// CPU is a difference between refreshes, so the first reading has nothing
    /// to compare against and would read as zero rather than as unknown
    sampled_once: Mutex<bool>,
}

impl ProcessMonitor {
    pub fn new() -> Self {
        let pid = get_current_pid().ok();

        if pid.is_none() {
            tracing::warn!("PROCESS_METRICS: could not determine our own pid, metrics unavailable");
        }

        Self {
            system: Mutex::new(System::new()),
            pid,
            sampled_once: Mutex::new(false),
        }
    }

    /// Refresh and read
    ///
    /// Call no more often than `sysinfo::MINIMUM_CPU_UPDATE_INTERVAL`; below that
    /// the difference the CPU figure is derived from is too small to mean much.
    pub fn sample(&self) -> ProcessMetrics {
        let Some(pid) = self.pid else {
            return ProcessMetrics::default();
        };

        let mut system = match self.system.lock() {
            Ok(system) => system,
            Err(poisoned) => poisoned.into_inner(),
        };

        system.refresh_process_specifics(pid, ProcessRefreshKind::new().with_cpu().with_memory());

        let Some(process) = system.process(pid) else {
            return ProcessMetrics::default();
        };

        let mut sampled_once = match self.sampled_once.lock() {
            Ok(flag) => flag,
            Err(poisoned) => poisoned.into_inner(),
        };
        let cpu_ready = *sampled_once;
        *sampled_once = true;

        ProcessMetrics {
            cpu_percent: process.cpu_usage(),
            memory_bytes: process.memory(),
            cpu_ready,
        }
    }
}

impl Default for ProcessMonitor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_reading_admits_it_has_no_cpu_figure_yet() {
        let monitor = ProcessMonitor::new();

        let first = monitor.sample();
        assert!(!first.cpu_ready);

        std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);

        let second = monitor.sample();
        assert!(second.cpu_ready);
    }

    #[test]
    fn a_running_process_is_using_some_memory() {
        let monitor = ProcessMonitor::new();
        assert!(monitor.sample().memory_bytes > 0);
    }
}
