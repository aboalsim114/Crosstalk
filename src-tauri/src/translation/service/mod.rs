//! Translation service: every accepted transcript is translated into the
//! user's language (Langues › Ma langue, French by default) on a dedicated
//! low-priority thread, with a bounded queue, an LRU cache, the user's
//! glossary and the same purge as the speech pipeline.

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use super::glossary::Term;
use super::models;

mod cache;
mod types;
mod worker;

pub use super::engine::{Chain, Engine, EngineLoader};
pub use super::opus::OpusLoader;
pub use types::*;

use cache::Cache;
use worker::worker_loop;

/// A line waiting longer than this is shown untranslated (F-34).
pub const MAX_WAIT: Duration = Duration::from_secs(5);
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

struct Pair {
    state: PairState,
    error: Option<String>,
    load_ms: Option<u32>,
}

struct Inner {
    events: Arc<dyn TranslationEvents>,
    loader: Arc<dyn EngineLoader>,
    models_dir: PathBuf,
    config: Mutex<TranslationConfig>,
    /// Per source language, for the current target.
    pairs: Mutex<HashMap<String, Pair>>,
    queue: Mutex<VecDeque<Job>>,
    wake: Condvar,
    cache: Mutex<Cache>,
    /// Callouts of the map being played, kept as spoken (F-36).
    map_terms: Mutex<Vec<Term>>,
    generation: AtomicU64,
    /// Asks the worker to (re)load the installed models.
    reload: AtomicBool,
    /// Asks the worker to drop its engines before reloading (online LLM changed).
    rebuild: AtomicBool,
    shutdown: AtomicBool,
}

pub struct TranslationService {
    inner: Arc<Inner>,
}

impl TranslationService {
    pub fn start(
        events: Arc<dyn TranslationEvents>,
        loader: Arc<dyn EngineLoader>,
        models_dir: PathBuf,
        config: TranslationConfig,
    ) -> Arc<Self> {
        let pairs = pairs_for(&config);
        let inner = Arc::new(Inner {
            events,
            loader,
            models_dir,
            config: Mutex::new(config),
            pairs: Mutex::new(pairs),
            queue: Mutex::new(VecDeque::new()),
            wake: Condvar::new(),
            cache: Mutex::new(Cache::default()),
            map_terms: Mutex::new(Vec::new()),
            generation: AtomicU64::new(0),
            reload: AtomicBool::new(true),
            rebuild: AtomicBool::new(false),
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
            *c = config.clone();
            (g, l)
        };
        if changed_glossary {
            *self.inner.cache.lock().unwrap() = Cache::default();
        }
        if changed_langs {
            // Pairs of the new languages, keeping the state of those that stay.
            let mut pairs = self.inner.pairs.lock().unwrap();
            let mut next = pairs_for(&config);
            for (k, v) in next.iter_mut() {
                if let Some(old) = pairs.remove(k) {
                    if old.state == PairState::Ready || old.state == PairState::Loading {
                        // Ready for the old target only: reloaded for the new one.
                        continue;
                    }
                    *v = old;
                }
            }
            *pairs = next;
            drop(pairs);
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

    /// The online LLM was configured, enabled, disabled or got a new key:
    /// translations made by the previous setup leave the cache and every route
    /// is rebuilt (a route without a local model exists only while the LLM is active).
    pub fn engines_changed(&self) -> TranslationStatus {
        *self.inner.cache.lock().unwrap() = Cache::default();
        self.inner.rebuild.store(true, Ordering::SeqCst);
        self.rescan()
    }

    /// Looks for newly installed models (and loads them).
    pub fn rescan(&self) -> TranslationStatus {
        self.inner.reload.store(true, Ordering::SeqCst);
        self.inner.wake.notify_all();
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

    /// Live words of a voice line: translated quickly when the worker is idle;
    /// a newer live line replaces a pending one, final lines go first.
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
            .unwrap_or_else(|_| failure(0, lang, text, "moteur indisponible", 0))
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

fn pairs_for(config: &TranslationConfig) -> HashMap<String, Pair> {
    config
        .translated()
        .into_iter()
        .map(|s| {
            (
                s,
                // Not examined yet: the worker says Missing or Ready once it
                // has looked. Missing here showed "modèles absents" (and a
                // WARN in the journal) for seconds at every start.
                Pair {
                    state: PairState::Loading,
                    error: None,
                    load_ms: None,
                },
            )
        })
        .collect()
}

impl Drop for TranslationService {
    fn drop(&mut self) {
        self.inner.shutdown.store(true, Ordering::SeqCst);
        self.inner.wake.notify_all();
    }
}

impl Inner {
    fn status(&self) -> TranslationStatus {
        let pairs = self.pairs.lock().unwrap();
        let config = self.config.lock().unwrap().clone();
        TranslationStatus {
            pairs: config
                .translated()
                .into_iter()
                .filter_map(|src| {
                    let s = pairs.get(&src)?;
                    let repo = models::route(&src, &config.target)
                        .unwrap_or_default()
                        .iter()
                        .map(|p| p.repo)
                        .collect::<Vec<_>>()
                        .join(" + ");
                    Some(PairStatus {
                        target: config.target.clone(),
                        source: src,
                        repo,
                        state: s.state,
                        error: s.error.clone(),
                        load_ms: s.load_ms,
                    })
                })
                .collect(),
            config,
            models_dir: self.models_dir.display().to_string(),
            queued: self.queue.lock().unwrap().len(),
        }
    }

    fn set_pair(
        &self,
        source: &str,
        state: PairState,
        error: Option<String>,
        load_ms: Option<u32>,
    ) {
        {
            let mut pairs = self.pairs.lock().unwrap();
            // The languages may have changed meanwhile.
            let Some(p) = pairs.get_mut(source) else {
                return;
            };
            p.state = state;
            p.error = error;
            if load_ms.is_some() || state != PairState::Ready {
                p.load_ms = load_ms;
            }
        }
        self.events.status(&self.status());
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
