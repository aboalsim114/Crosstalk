//! Speech statistics (Audio & IA, diagnostic): segments, rejections,
//! latency percentiles, real-time factor and CPU.

use std::collections::{BTreeMap, VecDeque};

use crate::cpu::CpuMeter;
use crate::speech::types::{RejectReason, SpeechStats};

const LATENCY_WINDOW: usize = 100;

#[derive(Default)]
pub(super) struct StatsAcc {
    pub(super) segments: u64,
    pub(super) transcribed: u64,
    pub(super) rejected: BTreeMap<RejectReason, u64>,
    pub(super) latencies: VecDeque<u32>,
    pub(super) last_rtf: Option<f32>,
    pub(super) cpu: CpuMeter,
    pub(super) cpu_percent: Option<f32>,
}

impl StatsAcc {
    pub(super) fn reject(&mut self, r: RejectReason) {
        *self.rejected.entry(r).or_default() += 1;
    }

    /// A transcript shown (final): counted, and its latency kept for the
    /// percentiles (last `LATENCY_WINDOW` lines).
    pub(super) fn transcribed(&mut self, latency_ms: u32) {
        self.transcribed += 1;
        self.latencies.push_back(latency_ms);
        if self.latencies.len() > LATENCY_WINDOW {
            self.latencies.pop_front();
        }
    }

    pub(super) fn snapshot(&self, dropped_blocks: u64) -> SpeechStats {
        let mut sorted: Vec<u32> = self.latencies.iter().copied().collect();
        sorted.sort_unstable();
        let pct = |q: f32| {
            (!sorted.is_empty())
                .then(|| sorted[((q * sorted.len() as f32) as usize).min(sorted.len() - 1)])
        };
        SpeechStats {
            segments: self.segments,
            transcribed: self.transcribed,
            rejected: self.rejected.clone(),
            dropped_blocks,
            latency_p50_ms: pct(0.5),
            latency_p95_ms: pct(0.95),
            last_rtf: self.last_rtf,
            cpu_percent: self.cpu_percent,
            focus_db: None,
        }
    }
}
