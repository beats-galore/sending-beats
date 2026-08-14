// Threads for the stages between the two realtime callbacks
//
// The capture and render callbacks run on threads CoreAudio schedules for
// realtime work. Everything between them was an ordinary task on the shared
// tokio runtime, competing with IPC, database work and metering, and being made
// to wait behind them for milliseconds at a time. That wait is why every handoff
// in the pipeline has to keep a cushion, and the cushion is most of the latency.
//
// These stages get their own threads instead, told what deadline they are working
// to, so the scheduler treats them like the audio work they are.

use std::thread::JoinHandle;
use std::time::Duration;

#[cfg(target_os = "macos")]
use tracing::{info, warn};

/// Spawn a thread that runs audio work to a deadline
///
/// `period` is how often the work must happen — a hardware buffer's worth of
/// time. The kernel is told the thread needs to run once per period and finish
/// within it, which is what stops it being queued behind ordinary work.
pub fn spawn(name: &str, period: Duration, work: impl FnOnce() + Send + 'static) -> JoinHandle<()> {
    let thread_name = name.to_string();

    std::thread::Builder::new()
        .name(thread_name.clone())
        .spawn(move || {
            apply_realtime_policy(&thread_name, period);
            work();
        })
        .expect("spawning an audio thread")
}

/// Ask the kernel to schedule this thread against an audio deadline
///
/// Advisory. A refusal costs latency rather than correctness, so it is logged
/// and the thread runs on anyway.
#[cfg(target_os = "macos")]
fn apply_realtime_policy(name: &str, period: Duration) {
    let Some(ticks_per_nano) = timebase_ratio() else {
        warn!(
            "⚠️ {}: no mach timebase, '{}' stays on default scheduling",
            "REALTIME_THREAD".on_bright_black().white(),
            name
        );
        return;
    };

    let period_nanos = period.as_nanos() as f64;
    let period_ticks = (period_nanos * ticks_per_nano) as u32;

    // Room for the work to take a fifth of its period and still be counted as
    // meeting the deadline. Mixing a block measures in tens of microseconds
    // against periods measured in milliseconds, so this is generous; asking for
    // more would make the kernel reserve capacity nothing uses.
    let computation_ticks = period_ticks / 5;

    let policy = libc::thread_time_constraint_policy {
        period: period_ticks,
        computation: computation_ticks,
        // The work is worth nothing late, so the deadline is the period itself
        constraint: period_ticks,
        // Preemptible: this is not so critical that it should be able to starve
        // the rest of the system, and CoreAudio's own callbacks matter more
        preemptible: 1,
    };

    let result = unsafe {
        libc::thread_policy_set(
            libc::mach_thread_self(),
            libc::THREAD_TIME_CONSTRAINT_POLICY as libc::thread_policy_flavor_t,
            &policy as *const _ as libc::thread_policy_t,
            libc::THREAD_TIME_CONSTRAINT_POLICY_COUNT,
        )
    };

    if result == libc::KERN_SUCCESS {
        info!(
            "⏱️ {}: '{}' scheduled to a {:.2}ms deadline",
            "REALTIME_THREAD".on_bright_black().white(),
            name,
            period.as_secs_f64() * 1000.0
        );
    } else {
        warn!(
            "⚠️ {}: '{}' refused a realtime deadline (kern {}), staying on default scheduling",
            "REALTIME_THREAD".on_bright_black().white(),
            name,
            result
        );
    }
}

#[cfg(not(target_os = "macos"))]
fn apply_realtime_policy(_name: &str, _period: Duration) {}

/// Mach ticks per nanosecond, which the time-constraint policy is expressed in
#[cfg(target_os = "macos")]
fn timebase_ratio() -> Option<f64> {
    let mut timebase = libc::mach_timebase_info { numer: 0, denom: 0 };

    if unsafe { libc::mach_timebase_info(&mut timebase) } != libc::KERN_SUCCESS {
        return None;
    }
    if timebase.numer == 0 {
        return None;
    }

    Some(timebase.denom as f64 / timebase.numer as f64)
}

#[cfg(target_os = "macos")]
use colored::*;

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    #[test]
    fn work_runs_whether_or_not_the_deadline_is_granted() {
        let ran = Arc::new(AtomicBool::new(false));
        let flag = ran.clone();

        let handle = spawn("test-audio", Duration::from_millis(3), move || {
            flag.store(true, Ordering::SeqCst);
        });

        handle.join().expect("thread joins");
        assert!(ran.load(Ordering::SeqCst));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn the_timebase_is_available_on_this_machine() {
        assert!(timebase_ratio().is_some());
    }
}
