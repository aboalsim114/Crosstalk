//! Translation service: every accepted line (voice transcript, chat message)
//! is translated into the user's language (Langues › Ma langue) by the LLM
//! the user connected, on a dedicated low-priority thread, with a bounded
//! queue, an LRU cache, the user's glossary and the same purge as the speech
//! pipeline. The audio thread, Tauri's main thread and the interface never
//! wait for an answer: results arrive as events.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use super::engine::Translator;
use super::glossary::Term;

mod cache;
mod types;
mod worker;

pub use types::*;

use cache::Cache;
use worker::worker_loop;

/// A line waiting longer than this is shown untranslated, with the reason (F-34).
pub const MAX_WAIT: Duration = Duration::from_secs(8);
pub const QUEUE_CAPACITY: usize = 8;
pub const CACHE_CAPACITY: usize = 1000;

pub trait TranslationEvents: Send + Sync {
    fn status(&self, status: &TranslationStatus);
    fn result(&self, translation: &Translation);
}

struct Job {
    id: u64,
    lang: String,
    text: String,
    queued: Instant,
    generation: u64,
    reply: Option<std::sync::mpsc::Sender<Translation>>,
    partial: bool,
}

struct Inner {
    events: Arc<dyn TranslationEvents>,
    translator: Arc<dyn Translator>,
    config: Mutex<TranslationConfig>,
    queue: Mutex<VecDeque<Job>>,
    wake: Condvar,
    cache: Mutex<Cache>,
    /// Callouts of the map being played, kept as spoken (F-36).
    map_terms: Mutex<Vec<Term>>,
    generation: AtomicU64,
    shutdown: AtomicBool,
}

pub struct TranslationService {
    inner: Arc<Inner>,
}

impl TranslationService {
    pub fn start(
        events: Arc<dyn TranslationEvents>,
        translator: Arc<dyn Translator>,
        config: TranslationConfig,
    ) -> Arc<Self> {
        let inner = Arc::new(Inner {
            events,
            translator,
            config: Mutex::new(config.sanitized()),
            queue: Mutex::new(VecDeque::new()),
            wake: Condvar::new(),
            cache: Mutex::new(Cache::default()),
            map_terms: Mutex::new(Vec::new()),
            generation: AtomicU64::new(0),
            shutdown: AtomicBool::new(false),
        });
        let worker = inner.clone();
        std::thread::Builder::new()
            .name("translate".into())
            .spawn(move || worker_loop(&worker))
            .expect("translation thread");
        Arc::new(Self { inner })
    }

    pub fn status(&self) -> TranslationStatus {
        self.inner.status()
    }

    pub fn configure(&self, config: TranslationConfig) -> TranslationStatus {
        let config = config.sanitized();
        let (changed_glossary, changed_langs) = {
            let mut c = self.inner.config.lock().unwrap();
            let g = c.glossary != config.glossary;
            let l = c.target != config.target || c.sources != config.sources;
            *c = config;
            (g, l)
        };
        if changed_glossary {
            *self.inner.cache.lock().unwrap() = Cache::default();
        }
        if changed_langs {
            // Lines queued for the old target would come back in the wrong language.
            self.purge();
        }
        self.rescan()
    }

    /// Callouts of the map being played: kept as spoken, after the user's own
    /// glossary (which wins for a term it already defines).
    pub fn set_map_terms(&self, callouts: Vec<String>) {
        let terms: Vec<Term> = callouts
            .into_iter()
            .map(|source| Term {
                source,
                target: None,
            })
            .collect();
        let mut current = self.inner.map_terms.lock().unwrap();
        if *current != terms {
            *current = terms;
            *self.inner.cache.lock().unwrap() = Cache::default();
        }
    }

    /// The LLM was connected, disconnected, changed or got a new key:
    /// translations of the previous setup leave the cache.
    pub fn engines_changed(&self) -> TranslationStatus {
        *self.inner.cache.lock().unwrap() = Cache::default();
        self.rescan()
    }

    /// Publishes the current state (engine ready or not).
    pub fn rescan(&self) -> TranslationStatus {
        let st = self.inner.status();
        self.inner.events.status(&st);
        st
    }

    /// Queues a recognized line; the result arrives as a `Translation` event.
    pub fn submit(&self, id: u64, lang: &str, text: &str) {
        self.push(Job {
            id,
            lang: lang.into(),
            text: text.into(),
            queued: Instant::now(),
            generation: self.inner.generation.load(Ordering::SeqCst),
            reply: None,
            partial: false,
        });
    }

    /// Live words of a voice line: a newer live line replaces a pending one,
    /// final lines go first, and the LLM decides whether it is worth a call
    /// (never towards a cloud provider: see `LlmClient::translate_partial`).
    pub fn submit_partial(&self, id: u64, lang: &str, text: &str) {
        {
            let mut q = self.inner.queue.lock().unwrap();
            q.retain(|j| !j.partial);
            q.push_back(Job {
                id,
                lang: lang.into(),
                text: text.into(),
                queued: Instant::now(),
                generation: self.inner.generation.load(Ordering::SeqCst),
                reply: None,
                partial: true,
            });
        }
        self.inner.wake.notify_all();
    }

    /// Translates now and waits (manual test from the UI, bench).
    pub fn translate_blocking(&self, lang: &str, text: &str) -> Translation {
        let (tx, rx) = std::sync::mpsc::channel();
        self.push(Job {
            id: 0,
            lang: lang.into(),
            text: text.into(),
            queued: Instant::now(),
            generation: self.inner.generation.load(Ordering::SeqCst),
            reply: Some(tx),
            partial: false,
        });
        rx.recv_timeout(Duration::from_secs(60))
            .unwrap_or_else(|_| failure(0, lang, text, "traduction trop longue", 0))
    }

    /// Drops queued lines (capture stopped, emergency stop, CS2 closed).
    pub fn purge(&self) {
        self.inner.generation.fetch_add(1, Ordering::SeqCst);
        self.inner
            .queue
            .lock()
            .unwrap()
            .retain(|j| j.reply.is_some());
    }

    fn push(&self, job: Job) {
        let dropped = {
            let mut q = self.inner.queue.lock().unwrap();
            q.push_back(job);
            if q.len() > QUEUE_CAPACITY {
                q.iter()
                    .position(|j| j.reply.is_none())
                    .and_then(|i| q.remove(i))
            } else {
                None
            }
        };
        if let Some(j) = dropped {
            let ms = j.queued.elapsed().as_millis() as u32;
            self.inner
                .events
                .result(&failure(j.id, &j.lang, &j.text, "file pleine", ms));
        }
        self.inner.wake.notify_all();
    }
}

impl Drop for TranslationService {
    fn drop(&mut self) {
        self.inner.shutdown.store(true, Ordering::SeqCst);
        self.inner.wake.notify_all();
    }
}

impl Inner {
    fn status(&self) -> TranslationStatus {
        let problem = self.translator.problem();
        TranslationStatus {
            config: self.config.lock().unwrap().clone(),
            ready: problem.is_none(),
            problem,
            queued: self.queue.lock().unwrap().len(),
        }
    }
}

fn failure(id: u64, lang: &str, text: &str, why: &str, queue_ms: u32) -> Translation {
    Translation {
        id,
        lang: lang.into(),
        original: text.into(),
        translated: None,
        error: Some(why.into()),
        ms: 0,
        queue_ms,
        cached: false,
        glossary: true,
        partial: false,
        target: String::new(),
        engine: None,
    }
}

#[cfg(test)]
mod tests;
