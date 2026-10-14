//! Translation service: every accepted transcript is translated into the
//! user's language (Langues › Ma langue, French by default) on a dedicated
//! low-priority thread, with a bounded queue, an LRU cache, the user's
//! glossary and the same purge as the speech pipeline.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use super::callouts;
use super::glossary::{self, Term};
use super::models;

/// A line waiting longer than this is shown untranslated (F-34).
pub const MAX_WAIT: Duration = Duration::from_secs(5);
pub const QUEUE_CAPACITY: usize = 8;
pub const CACHE_CAPACITY: usize = 1000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TranslationConfig {
    pub enabled: bool,
    #[serde(default)]
    pub glossary: Vec<Term>,
    /// The user's language: everything is translated into it.
    #[serde(default = "default_target")]
    pub target: String,
    /// The players' languages (lines in other languages are shown as is).
    #[serde(default = "default_sources")]
    pub sources: Vec<String>,
}

fn default_target() -> String {
    models::DEFAULT_TARGET.into()
}

fn default_sources() -> Vec<String> {
    models::DEFAULT_SOURCES
        .iter()
        .map(|s| s.to_string())
        .collect()
}

impl Default for TranslationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            glossary: Vec::new(),
            target: default_target(),
            sources: default_sources(),
        }
    }
}

impl TranslationConfig {
    /// Unknown languages dropped; an unsupported target falls back to French.
    pub fn sanitized(mut self) -> Self {
        if !models::is_target(&self.target) {
            self.target = default_target();
        }
        let mut out: Vec<String> = Vec::new();
        for l in self.sources {
            if models::is_language(&l) && !out.contains(&l) {
                out.push(l);
            }
        }
        self.sources = out;
        self
    }

    /// Languages to translate: the players' ones, except the user's own.
    pub fn translated(&self) -> Vec<String> {
        self.sources
            .iter()
            .filter(|s| **s != self.target && models::route(s, &self.target).is_some())
            .cloned()
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PairState {
    Missing,
    Loading,
    Ready,
    Error,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PairStatus {
    pub source: String,
    pub target: String,
    /// Repositories of the route ("A + B" when it goes through English).
    pub repo: String,
    pub state: PairState,
    pub error: Option<String>,
    pub load_ms: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationStatus {
    pub config: TranslationConfig,
    pub pairs: Vec<PairStatus>,
    pub models_dir: String,
    pub queued: usize,
}

/// Result for one line, matched to the transcript by `id`.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Translation {
    pub id: u64,
    pub lang: String,
    pub original: String,
    /// French text, or `None` with `error` set: the original stays displayed.
    pub translated: Option<String>,
    pub error: Option<String>,
    /// Translation time (queue wait excluded).
    pub ms: u32,
    pub queue_ms: u32,
    pub cached: bool,
    /// False when glossary terms could not be protected in this line.
    pub glossary: bool,
    /// Quick translation of a live (partial) line, replaced by the final one.
    #[serde(default)]
    pub partial: bool,
    /// Language of `translated` (the user's language when it was sent).
    #[serde(default)]
    pub target: String,
}

pub trait Engine: Send {
    fn translate(&mut self, text: &str) -> Result<String, String>;
    /// Faster, lower quality (live lines): greedy decoding.
    fn translate_fast(&mut self, text: &str) -> Result<String, String> {
        self.translate(text)
    }
}

pub trait EngineLoader: Send + Sync {
    fn installed(&self, dir: &Path, source: &str, target: &str) -> bool;
    fn load(&self, dir: &Path, source: &str, target: &str) -> Result<Box<dyn Engine>, String>;
}

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

#[derive(Default)]
struct Cache {
    map: HashMap<(String, String), String>,
    order: VecDeque<(String, String)>,
}

impl Cache {
    fn get(&mut self, key: &(String, String)) -> Option<String> {
        let v = self.map.get(key)?.clone();
        if let Some(i) = self.order.iter().position(|k| k == key) {
            let k = self.order.remove(i).expect("present");
            self.order.push_back(k);
        }
        Some(v)
    }

    fn put(&mut self, key: (String, String), value: String) {
        if self.map.insert(key.clone(), value).is_none() {
            self.order.push_back(key);
            if self.order.len() > CACHE_CAPACITY {
                if let Some(old) = self.order.pop_front() {
                    self.map.remove(&old);
                }
            }
        }
    }
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
                Pair {
                    state: PairState::Missing,
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
    }
}

type Engines = HashMap<(String, String), Box<dyn Engine>>;

fn worker_loop(inner: &Inner) {
    crate::cpu::lower_current_thread_priority();
    let mut engines: Engines = HashMap::new();
    while !inner.shutdown.load(Ordering::SeqCst) {
        if inner.reload.swap(false, Ordering::SeqCst) {
            load_installed(inner, &mut engines);
        }
        let job = {
            let mut q = inner.queue.lock().unwrap();
            loop {
                if inner.shutdown.load(Ordering::SeqCst) || inner.reload.load(Ordering::SeqCst) {
                    break None;
                }
                // Final lines before the pending live one.
                let next = q
                    .iter()
                    .position(|j| !j.partial)
                    .or((!q.is_empty()).then_some(0));
                if let Some(j) = next.and_then(|i| q.remove(i)) {
                    break Some(j);
                }
                q = inner
                    .wake
                    .wait_timeout(q, Duration::from_millis(500))
                    .unwrap()
                    .0;
            }
        };
        let Some(job) = job else { continue };
        let result = run(inner, &mut engines, &job);
        match (&job.reply, result) {
            (Some(tx), Some(t)) => {
                let _ = tx.send(t);
            }
            (None, Some(t)) => inner.events.result(&t),
            _ => {}
        }
    }
}

fn load_installed(inner: &Inner, engines: &mut Engines) {
    let config = inner.config.lock().unwrap().clone();
    let wanted: Vec<(String, String)> = config
        .translated()
        .into_iter()
        .map(|s| (s, config.target.clone()))
        .collect();
    // Models of languages no longer chosen free their memory.
    engines.retain(|k, _| wanted.contains(k));
    for (src, tgt) in wanted {
        if engines.contains_key(&(src.clone(), tgt.clone())) {
            inner.set_pair(&src, PairState::Ready, None, None);
            continue;
        }
        if !inner.loader.installed(&inner.models_dir, &src, &tgt) {
            inner.set_pair(&src, PairState::Missing, None, None);
            continue;
        }
        inner.set_pair(&src, PairState::Loading, None, None);
        let t = Instant::now();
        match inner.loader.load(&inner.models_dir, &src, &tgt) {
            Ok(e) => {
                inner.set_pair(
                    &src,
                    PairState::Ready,
                    None,
                    Some(t.elapsed().as_millis() as u32),
                );
                engines.insert((src, tgt), e);
            }
            Err(e) => inner.set_pair(&src, PairState::Error, Some(e), None),
        }
    }
}

/// Translates one job; `None` when it was purged (nothing is published).
fn run(inner: &Inner, engines: &mut Engines, job: &Job) -> Option<Translation> {
    let manual = job.reply.is_some();
    if !manual && job.generation != inner.generation.load(Ordering::SeqCst) {
        return None;
    }
    let queue_ms = job.queued.elapsed().as_millis() as u32;
    let mut config = inner.config.lock().unwrap().clone();
    let target = config.target.clone();
    let base = || Translation {
        target: target.clone(),
        ..failure(job.id, &job.lang, &job.text, "", queue_ms)
    };
    let fail = |why: &str| {
        Some(Translation {
            error: Some(why.into()),
            ..base()
        })
    };
    for t in inner.map_terms.lock().unwrap().iter() {
        if !config
            .glossary
            .iter()
            .any(|g| g.source.eq_ignore_ascii_case(&t.source))
        {
            config.glossary.push(t.clone());
        }
    }
    if !config.enabled && !manual {
        return fail("traduction désactivée");
    }
    if job.lang == target {
        // F-32: already in the user's language, shown as is.
        return Some(Translation {
            translated: Some(job.text.clone()),
            ..base()
        });
    }
    // Known Counter-Strike callouts: exact, instant, no model needed (F-36).
    // Their renderings are French: only when translating into French.
    if target == "fr" {
        if let Some(text) = callouts::translate(&job.lang, &job.text) {
            return Some(Translation {
                translated: Some(text),
                partial: job.partial,
                ..base()
            });
        }
    }
    if models::route(&job.lang, &target).is_none() {
        return fail("langue non gérée");
    }
    // A language the user did not choose (Langues) is shown as is.
    if !manual && !config.sources.contains(&job.lang) {
        return fail("langue non choisie (Langues)");
    }
    let route_key = (job.lang.clone(), target.clone());
    // A live line older than 2 s has been replaced on screen: skip it.
    if job.partial && job.queued.elapsed() > Duration::from_secs(2) {
        return None;
    }
    if !manual && job.queued.elapsed() > MAX_WAIT {
        return fail("moteur indisponible (délai dépassé)");
    }
    let Some(engine) = engines.get_mut(&route_key) else {
        if job.partial {
            return None; // the final line will say why
        }
        // Present but unreadable is not the same as absent.
        let broken = inner
            .pairs
            .lock()
            .unwrap()
            .get(&job.lang)
            .is_some_and(|p| p.state == PairState::Error);
        if broken {
            return fail("modèle de traduction illisible (voir Audio & IA)");
        }
        return fail("modèle de traduction absent");
    };
    let key = (format!("{}>{target}", job.lang), cache_key(&job.text));
    let started = Instant::now();
    if job.partial {
        let out = glossary::translate(&job.text, &config.glossary, |t| engine.translate_fast(t));
        if job.generation != inner.generation.load(Ordering::SeqCst) {
            return None;
        }
        return match out {
            Ok((text, protected)) if !callouts::clean_output(&text).is_empty() => {
                Some(Translation {
                    translated: Some(callouts::clean_output(&text)),
                    error: None,
                    ms: started.elapsed().as_millis() as u32,
                    glossary: protected,
                    partial: true,
                    ..base()
                })
            }
            _ => None,
        };
    }
    if let Some(hit) = inner.cache.lock().unwrap().get(&key) {
        return Some(Translation {
            translated: Some(hit),
            error: None,
            cached: true,
            ms: started.elapsed().as_millis() as u32,
            ..base()
        });
    }
    let outcome = glossary::translate(&job.text, &config.glossary, |t| engine.translate(t));
    let ms = started.elapsed().as_millis() as u32;
    if !manual && job.generation != inner.generation.load(Ordering::SeqCst) {
        return None;
    }
    match outcome.map(|(t, p)| (callouts::clean_output(&t), p)) {
        Ok((text, protected)) if !text.trim().is_empty() => {
            inner.cache.lock().unwrap().put(key, text.clone());
            Some(Translation {
                id: job.id,
                lang: job.lang.clone(),
                original: job.text.clone(),
                translated: Some(text),
                error: None,
                ms,
                queue_ms,
                cached: false,
                glossary: protected,
                partial: false,
                target: target.clone(),
            })
        }
        Ok(_) => fail("traduction vide"),
        Err(e) => fail(&format!("moteur indisponible : {e}")),
    }
}

/// Repeated callouts ("gg", "rush B") hit the cache whatever their case or
/// spacing (the cache is cleared when the glossary changes).
fn cache_key(text: &str) -> String {
    let words: Vec<String> = text.split_whitespace().map(str::to_lowercase).collect();
    words.join(" ")
}

/// Real models from `<models dir>/opus-mt-<src>-<tgt>/`, one or two in a
/// row (through English when there is no direct model).
pub struct OpusLoader;

impl EngineLoader for OpusLoader {
    fn installed(&self, dir: &Path, source: &str, target: &str) -> bool {
        models::route_installed(dir, source, target)
    }

    fn load(&self, dir: &Path, source: &str, target: &str) -> Result<Box<dyn Engine>, String> {
        let route = models::route(source, target).ok_or("langue non gérée")?;
        let mut steps: Vec<Box<dyn Engine>> = Vec::new();
        for p in route {
            let m =
                super::marian::Marian::load(&models::pair_dir(dir, p), super::marian::Mode::Fast)
                    .map_err(|e| format!("{} : {e}", p.dir))?;
            steps.push(Box::new(m));
        }
        Ok(if steps.len() == 1 {
            steps.pop().expect("one")
        } else {
            Box::new(Chain(steps))
        })
    }
}

/// Models applied one after the other (source → English → target).
pub struct Chain(pub Vec<Box<dyn Engine>>);

impl Engine for Chain {
    fn translate(&mut self, text: &str) -> Result<String, String> {
        self.0
            .iter_mut()
            .try_fold(text.to_string(), |t, e| e.translate(&t))
    }
    fn translate_fast(&mut self, text: &str) -> Result<String, String> {
        self.0
            .iter_mut()
            .try_fold(text.to_string(), |t, e| e.translate_fast(&t))
    }
}

impl Engine for super::marian::Marian {
    fn translate(&mut self, text: &str) -> Result<String, String> {
        super::marian::Marian::translate(self, text)
    }
    fn translate_fast(&mut self, text: &str) -> Result<String, String> {
        self.translate_with(text, 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::{channel, Receiver, Sender};

    struct Upper(Arc<AtomicU64>, Duration);
    impl Engine for Upper {
        fn translate(&mut self, text: &str) -> Result<String, String> {
            self.0.fetch_add(1, Ordering::SeqCst);
            std::thread::sleep(self.1);
            Ok(format!("FR:{text}"))
        }
    }

    struct Loader {
        installed: Vec<&'static str>,
        calls: Arc<AtomicU64>,
        delay: Duration,
    }
    impl EngineLoader for Loader {
        fn installed(&self, _: &Path, source: &str, _: &str) -> bool {
            self.installed.contains(&source)
        }
        fn load(&self, _: &Path, _: &str, _: &str) -> Result<Box<dyn Engine>, String> {
            Ok(Box::new(Upper(self.calls.clone(), self.delay)))
        }
    }

    struct Events(Mutex<Sender<Translation>>);
    impl TranslationEvents for Events {
        fn status(&self, _: &TranslationStatus) {}
        fn result(&self, t: &Translation) {
            let _ = self.0.lock().unwrap().send(t.clone());
        }
    }

    fn service(
        installed: Vec<&'static str>,
        delay_ms: u64,
    ) -> (
        Arc<TranslationService>,
        Receiver<Translation>,
        Arc<AtomicU64>,
    ) {
        let (tx, rx) = channel();
        let calls = Arc::new(AtomicU64::new(0));
        let svc = TranslationService::start(
            Arc::new(Events(Mutex::new(tx))),
            Arc::new(Loader {
                installed,
                calls: calls.clone(),
                delay: Duration::from_millis(delay_ms),
            }),
            PathBuf::from("models"),
            TranslationConfig::default(),
        );
        let end = Instant::now() + Duration::from_secs(5);
        while svc
            .status()
            .pairs
            .iter()
            .any(|p| p.state == PairState::Loading)
            || svc.inner.reload.load(Ordering::SeqCst)
        {
            assert!(Instant::now() < end);
            std::thread::sleep(Duration::from_millis(5));
        }
        (svc, rx, calls)
    }

    fn next(rx: &Receiver<Translation>) -> Translation {
        rx.recv_timeout(Duration::from_secs(5))
            .expect("translation event")
    }

    #[test]
    fn known_callouts_need_no_model() {
        let (svc, rx, calls) = service(vec![], 0);
        svc.submit(1, "en", "CT, go, go.");
        let t = next(&rx);
        assert_eq!(t.translated.as_deref(), Some("CT, on y va"));
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn subtitle_dashes_are_removed_from_the_model_output() {
        struct Dash;
        impl Engine for Dash {
            fn translate(&mut self, _: &str) -> Result<String, String> {
                Ok("- Deux vont sur B.".into())
            }
        }
        let mut engines: Engines = HashMap::new();
        engines.insert(("en".into(), "fr".into()), Box::new(Dash));
        let (svc, _rx, _) = service(vec![], 0);
        let job = Job {
            id: 1,
            lang: "en".into(),
            text: "Two are going B".into(),
            queued: Instant::now(),
            generation: 0,
            reply: None,
            partial: false,
        };
        let t = run(&svc.inner, &mut engines, &job).unwrap();
        assert_eq!(t.translated.as_deref(), Some("Deux vont sur B."));
    }

    #[test]
    fn live_lines_wait_for_final_ones_and_only_the_latest_is_kept() {
        let (svc, rx, _) = service(vec!["ru"], 150);
        svc.submit(1, "ru", "первая фраза");
        svc.submit_partial(2, "ru", "живая");
        svc.submit_partial(2, "ru", "живая строка");
        svc.submit(3, "ru", "вторая фраза");
        let order: Vec<(u64, bool, String)> = (0..3)
            .map(|_| next(&rx))
            .map(|t| (t.id, t.partial, t.translated.unwrap()))
            .collect();
        assert_eq!(
            order,
            vec![
                (1, false, "FR:первая фраза".into()),
                (3, false, "FR:вторая фраза".into()),
                (2, true, "FR:живая строка".into())
            ]
        );
        assert!(
            rx.recv_timeout(Duration::from_millis(400)).is_err(),
            "the replaced live line was translated"
        );
    }

    #[test]
    fn a_transcript_comes_back_in_french_then_from_the_cache() {
        let (svc, rx, calls) = service(vec!["ru", "en"], 0);
        assert!(svc
            .status()
            .pairs
            .iter()
            .all(|p| p.state == PairState::Ready));
        svc.submit(7, "ru", "Двое идут на B");
        let t = next(&rx);
        assert_eq!(
            (t.id, t.translated.as_deref(), t.cached),
            (7, Some("FR:Двое идут на B"), false)
        );
        svc.submit(8, "ru", "  двое   ИДУТ на b ");
        let t = next(&rx);
        assert_eq!(
            (t.id, t.translated.as_deref(), t.cached),
            (8, Some("FR:Двое идут на B"), true)
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn missing_model_or_disabled_translation_keeps_the_original() {
        let (svc, rx, _) = service(vec!["ru"], 0);
        let st = svc.status();
        assert_eq!(
            st.pairs.iter().find(|p| p.source == "en").unwrap().state,
            PairState::Missing
        );
        svc.submit(1, "en", "Two of them are going B");
        let t = next(&rx);
        assert_eq!(t.translated, None);
        assert_eq!(t.error.as_deref(), Some("modèle de traduction absent"));
        svc.configure(TranslationConfig {
            enabled: false,
            glossary: vec![],
            ..TranslationConfig::default()
        });
        svc.submit(2, "ru", "Двое");
        assert_eq!(next(&rx).error.as_deref(), Some("traduction désactivée"));
    }

    #[test]
    fn french_is_never_sent_to_a_model() {
        let (svc, rx, calls) = service(vec!["ru", "en"], 0);
        svc.submit(3, "fr", "on push B");
        let t = next(&rx);
        assert_eq!(t.translated.as_deref(), Some("on push B"));
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn purge_drops_queued_and_running_lines() {
        let (svc, rx, _) = service(vec!["ru"], 300);
        svc.submit(1, "ru", "раз");
        svc.submit(2, "ru", "два");
        std::thread::sleep(Duration::from_millis(50));
        svc.purge();
        assert!(rx.recv_timeout(Duration::from_millis(800)).is_err());
        svc.submit(3, "ru", "три");
        assert_eq!(next(&rx).id, 3);
    }

    #[test]
    fn a_full_queue_reports_the_oldest_line_untranslated() {
        let (svc, rx, _) = service(vec!["ru"], 200);
        for id in 0..=QUEUE_CAPACITY as u64 + 1 {
            svc.submit(id, "ru", &format!("фраза {id}"));
        }
        let dropped: Vec<Translation> =
            std::iter::from_fn(|| rx.recv_timeout(Duration::from_millis(100)).ok())
                .filter(|t| t.error.as_deref() == Some("file pleine"))
                .collect();
        assert!(!dropped.is_empty());
        assert!(dropped.iter().all(|t| t.translated.is_none()));
    }

    #[test]
    fn manual_test_waits_for_the_result_and_glossary_is_applied() {
        let (svc, _rx, _) = service(vec!["en"], 0);
        svc.configure(TranslationConfig {
            enabled: true,
            glossary: vec![Term {
                source: "smoke".into(),
                target: None,
            }],
            ..TranslationConfig::default()
        });
        let t = svc.translate_blocking("en", "throw Smoke");
        assert_eq!(t.translated.as_deref(), Some("FR:throw Smoke"));
        assert!(t.glossary);
    }

    #[test]
    fn map_callouts_are_kept_and_the_user_glossary_wins() {
        let (svc, _rx, calls) = service(vec!["en"], 0);
        svc.configure(TranslationConfig {
            enabled: true,
            glossary: vec![Term {
                source: "palace".into(),
                target: Some("palais".into()),
            }],
            ..TranslationConfig::default()
        });
        svc.translate_blocking("en", "go jungle");
        svc.translate_blocking("en", "go jungle");
        let before = calls.load(Ordering::SeqCst);
        // A new map clears the cache: the line is translated again.
        svc.set_map_terms(vec!["Jungle".into(), "Palace".into()]);
        let t = svc.translate_blocking("en", "go jungle");
        assert_eq!(calls.load(Ordering::SeqCst), before + 1);
        assert_eq!(t.translated.as_deref(), Some("FR:go jungle"));
        let t = svc.translate_blocking("en", "push palace");
        assert_eq!(t.translated.as_deref(), Some("FR:push palais"));
        // Same map again: the cache is kept.
        svc.set_map_terms(vec!["Jungle".into(), "Palace".into()]);
        svc.translate_blocking("en", "go jungle");
        assert_eq!(calls.load(Ordering::SeqCst), before + 2);
    }
    fn settle(svc: &TranslationService) {
        let end = Instant::now() + Duration::from_secs(5);
        while svc
            .status()
            .pairs
            .iter()
            .any(|p| p.state == PairState::Loading)
            || svc.inner.reload.load(Ordering::SeqCst)
        {
            assert!(Instant::now() < end);
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn a_russian_user_gets_the_other_languages_in_russian() {
        let (svc, rx, calls) = service(vec!["en", "fr"], 0);
        let st = svc.configure(TranslationConfig {
            target: "ru".into(),
            sources: vec!["en".into(), "fr".into(), "ru".into(), "xx".into()],
            ..TranslationConfig::default()
        });
        assert_eq!(
            st.config.sources,
            ["en", "fr", "ru"],
            "unknown language dropped"
        );
        // Both models are installed: a pair still Missing has not been
        // looked at yet by the worker (it clears `reload` before the first
        // pair is marked Loading), so wait for it too.
        let end = Instant::now() + Duration::from_secs(5);
        let st = loop {
            settle(&svc);
            let st = svc.status();
            if st.pairs.iter().all(|p| p.state != PairState::Missing) || Instant::now() > end {
                break st;
            }
            std::thread::sleep(Duration::from_millis(5));
        };
        let pairs: Vec<_> = st
            .pairs
            .iter()
            .map(|p| (p.source.as_str(), p.target.as_str(), p.state))
            .collect();
        assert_eq!(
            pairs,
            [
                ("en", "ru", PairState::Ready),
                ("fr", "ru", PairState::Ready)
            ]
        );
        // Their own language: shown as is.
        svc.submit(1, "ru", "Двое на B");
        let t = next(&rx);
        assert_eq!(
            (t.translated.as_deref(), t.target.as_str()),
            (Some("Двое на B"), "ru")
        );
        // The French callout lexicon does not answer into Russian: the model does.
        svc.submit(2, "en", "rush B");
        let t = next(&rx);
        assert_eq!(
            (t.translated.as_deref(), t.target.as_str()),
            (Some("FR:rush B"), "ru")
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        // A language not chosen stays as is.
        svc.submit(3, "de", "zwei auf B");
        assert_eq!(
            next(&rx).error.as_deref(),
            Some("langue non choisie (Langues)")
        );
    }

    #[test]
    fn changing_the_target_reloads_and_never_mixes_cached_lines() {
        let (svc, rx, calls) = service(vec!["en", "ru", "fr"], 0);
        svc.submit(1, "en", "they rotate to B with the AWP");
        assert!(next(&rx).translated.is_some());
        svc.configure(TranslationConfig {
            target: "de".into(),
            sources: vec!["en".into()],
            ..TranslationConfig::default()
        });
        settle(&svc);
        svc.submit(2, "en", "they rotate to B with the AWP");
        let t = next(&rx);
        assert!(
            !t.cached,
            "a cached French line must not come back in German"
        );
        assert_eq!(t.target, "de");
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        // An unsupported target falls back to French.
        let st = svc.configure(TranslationConfig {
            target: "pl".into(),
            ..TranslationConfig::default()
        });
        assert_eq!(st.config.target, "fr");
    }
}
