//! Automatic fallback to a lighter Whisper model during a capture.
//!
//! With CS2 taking the processor, a heavy model (whisper-small and up) can
//! fall so far behind that almost every segment expires in the queue: nothing
//! reaches the screen. When Whisper keeps lagging, the service switches to
//! whisper-base until the capture stops, then loads the chosen model again.
//! The user's setting is never changed.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// The lighter model used when the chosen one cannot keep up.
pub const FALLBACK_MODEL: &str = "base";
/// Lag signals needed within `LAG_WINDOW` to switch.
pub const LAG_SIGNALS: usize = 3;
pub const LAG_WINDOW: Duration = Duration::from_secs(60);
/// Whisper slower than this share of the audio duration is a lag signal.
pub const SLOW_RTF: f32 = 1.0;

/// Models light enough to never need a fallback.
pub fn is_light(model: &str) -> bool {
    matches!(model, "tiny" | "base")
}

/// One sign that Whisper cannot keep up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lag {
    /// A segment expired in the queue.
    Stale,
    /// The queue was full: the oldest segment was dropped.
    Overflow,
    /// Whisper took longer than the audio it transcribed.
    Slow,
}

/// Lag signals of the last minute, and why the service switched.
#[derive(Debug, Default)]
pub struct LagWatch {
    signals: VecDeque<(Instant, Lag)>,
}

impl LagWatch {
    pub fn note(&mut self, lag: Lag, now: Instant) {
        self.signals.push_back((now, lag));
        while self
            .signals
            .front()
            .is_some_and(|(t, _)| now.duration_since(*t) > LAG_WINDOW)
        {
            self.signals.pop_front();
        }
    }

    /// True when enough signals fell within the window.
    pub fn lagging(&self) -> bool {
        self.signals.len() >= LAG_SIGNALS
    }

    /// "2 segment(s) trop tard, 1 file pleine, 1 calcul plus long que la voix".
    pub fn summary(&self) -> String {
        let count = |k: Lag| self.signals.iter().filter(|(_, l)| *l == k).count();
        [
            (count(Lag::Stale), "segment(s) trop tard"),
            (count(Lag::Overflow), "file pleine"),
            (count(Lag::Slow), "calcul plus long que la voix"),
        ]
        .iter()
        .filter(|(n, _)| *n > 0)
        .map(|(n, what)| format!("{n} {what}"))
        .collect::<Vec<_>>()
        .join(", ")
    }

    pub fn clear(&mut self) {
        self.signals.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn three_signals_within_a_minute_mean_whisper_lags() {
        let t0 = Instant::now();
        let mut w = LagWatch::default();
        w.note(Lag::Stale, t0);
        w.note(Lag::Overflow, t0 + Duration::from_secs(10));
        assert!(!w.lagging());
        w.note(Lag::Slow, t0 + Duration::from_secs(20));
        assert!(w.lagging());
        assert_eq!(
            w.summary(),
            "1 segment(s) trop tard, 1 file pleine, 1 calcul plus long que la voix"
        );
        w.clear();
        assert!(!w.lagging());
    }

    #[test]
    fn signals_older_than_a_minute_are_forgotten() {
        let t0 = Instant::now();
        let mut w = LagWatch::default();
        w.note(Lag::Stale, t0);
        w.note(Lag::Stale, t0 + Duration::from_secs(40));
        w.note(Lag::Stale, t0 + Duration::from_secs(91));
        assert!(!w.lagging(), "the first one is 91 s old");
        assert_eq!(w.summary(), "2 segment(s) trop tard");
    }

    #[test]
    fn only_heavy_models_fall_back() {
        assert!(is_light("tiny") && is_light("base"));
        assert!(!is_light("small") && !is_light("medium") && !is_light("large-v3-turbo"));
    }
}
