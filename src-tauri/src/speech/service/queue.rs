//! The queue between the VAD thread and the Whisper thread.

use std::collections::VecDeque;
use std::sync::{Condvar, Mutex};
use std::time::Duration;

use super::{Job, QUEUE_CAPACITY};

#[derive(Default)]
pub(super) struct Jobs {
    finals: VecDeque<Job>,
    /// The latest partial request (an older one is useless).
    partial: Option<Job>,
}

/// Bounded queue of final segments (drops the oldest when full), plus one
/// partial slot served only when no final segment waits.
#[derive(Default)]
pub(super) struct JobQueue {
    pub(super) jobs: Mutex<Jobs>,
    pub(super) ready: Condvar,
}

impl JobQueue {
    pub(super) fn push(&self, job: Job) -> Option<Job> {
        let mut q = self.jobs.lock().unwrap();
        if q.partial
            .as_ref()
            .is_some_and(|p| p.utterance == job.utterance)
        {
            q.partial = None;
        }
        let dropped = if q.finals.len() >= QUEUE_CAPACITY {
            q.finals.pop_front()
        } else {
            None
        };
        q.finals.push_back(job);
        self.ready.notify_one();
        dropped
    }

    /// Asks for a partial transcript; false when final segments are waiting.
    pub(super) fn push_partial(&self, job: Job) -> bool {
        let mut q = self.jobs.lock().unwrap();
        if !q.finals.is_empty() {
            return false;
        }
        q.partial = Some(job);
        self.ready.notify_one();
        true
    }

    pub(super) fn pop(&self, timeout: Duration) -> Option<Job> {
        let q = self.jobs.lock().unwrap();
        let (mut q, _) = self
            .ready
            .wait_timeout_while(q, timeout, |q| q.finals.is_empty() && q.partial.is_none())
            .unwrap();
        q.finals.pop_front().or_else(|| q.partial.take())
    }

    pub(super) fn clear(&self) {
        let mut q = self.jobs.lock().unwrap();
        q.finals.clear();
        q.partial = None;
    }
}
