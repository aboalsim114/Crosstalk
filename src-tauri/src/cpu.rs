//! CPU usage of the Crosstalk core process (WASAPI capture, VAD and the recogniser all
//! run in it; the WebView2 UI has its own processes), and the priority of the
//! compute threads (speech, translation). Reads this process's own counters
//! only. A leaf module: it uses nothing else from the crate.

use std::time::{Duration, Instant};

/// Total user + kernel CPU time consumed by this process.
pub fn process_cpu_time() -> Option<Duration> {
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::FILETIME;
        use windows::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes};
        let (mut create, mut exit, mut kernel, mut user) = (
            FILETIME::default(),
            FILETIME::default(),
            FILETIME::default(),
            FILETIME::default(),
        );
        // SAFETY: the pseudo handle of the current process needs no closing and
        // the four out-pointers are valid for the call.
        unsafe {
            GetProcessTimes(
                GetCurrentProcess(),
                &mut create,
                &mut exit,
                &mut kernel,
                &mut user,
            )
        }
        .ok()?;
        let ticks = |f: FILETIME| ((f.dwHighDateTime as u64) << 32) | f.dwLowDateTime as u64;
        Some(Duration::from_nanos((ticks(kernel) + ticks(user)) * 100))
    }
    #[cfg(target_os = "linux")]
    {
        // Fields 14 and 15 of /proc/self/stat, in clock ticks (USER_HZ = 100).
        let stat = std::fs::read_to_string("/proc/self/stat").ok()?;
        let rest = &stat[stat.rfind(')')? + 2..];
        let fields: Vec<&str> = rest.split_whitespace().collect();
        let utime: u64 = fields.get(11)?.parse().ok()?;
        let stime: u64 = fields.get(12)?.parse().ok()?;
        Some(Duration::from_millis((utime + stime) * 10))
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        None
    }
}

/// Runs the calling thread below normal priority so the game keeps the CPU
/// when both compete. No-op outside Windows. Not for the recogniser: its worker
/// threads wait for the calling one at every step (see `speech::service`).
pub fn lower_current_thread_priority() {
    #[cfg(windows)]
    {
        use windows::Win32::System::Threading::{
            GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_BELOW_NORMAL,
        };
        // SAFETY: pseudo handle of the calling thread.
        let _ = unsafe { SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_BELOW_NORMAL) };
    }
}

/// Process CPU usage between two samples, in % of the whole machine.
pub struct CpuMeter {
    last: Option<(Instant, Duration)>,
    cpus: f32,
}

impl Default for CpuMeter {
    fn default() -> Self {
        Self {
            last: None,
            cpus: std::thread::available_parallelism()
                .map(|n| n.get() as f32)
                .unwrap_or(1.0),
        }
    }
}

impl CpuMeter {
    pub fn sample(&mut self) -> Option<f32> {
        let now = (Instant::now(), process_cpu_time()?);
        let prev = self.last.replace(now)?;
        let wall = now.0.duration_since(prev.0).as_secs_f32();
        if wall < 0.2 {
            self.last = Some(prev);
            return None;
        }
        let cpu = now.1.saturating_sub(prev.1).as_secs_f32();
        Some((cpu / wall / self.cpus * 100.0).clamp(0.0, 100.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measures_busy_work() {
        let Some(t0) = process_cpu_time() else {
            return; // unsupported platform
        };
        let start = Instant::now();
        let mut x = 0u64;
        while start.elapsed() < Duration::from_millis(150) {
            x = x.wrapping_mul(6364136223846793005).wrapping_add(1);
        }
        std::hint::black_box(x);
        let t1 = process_cpu_time().unwrap();
        assert!(t1 > t0);
    }

    #[test]
    fn meter_needs_two_samples() {
        let mut m = CpuMeter::default();
        assert_eq!(m.sample(), None);
        // Too close to the first sample: skipped, the first one is kept.
        assert_eq!(m.sample(), None);
        lower_current_thread_priority();
    }
}
