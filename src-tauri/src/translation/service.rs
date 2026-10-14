//! Translation service: every accepted transcript is translated into French on
//! a dedicated low-priority thread, with a bounded queue, an LRU cache, the
//! user's glossary and the same purge as the speech pipeline.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use super::callouts;
use super::glossary::{self, Term};
use super::llm::{Backend, EngineChoice, LlmStatus};
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
    /// TranslateGemma, Opus-MT or automatic (src/translation/llm).
    #[serde(default)]
    pub engine: EngineChoice,
    #[serde(default)]
    pub llm_backend: Backend,
    /// TranslateGemma's generation threads; 0: automatic.
    #[serde(default)]
    pub llm_threads: u32,
}

impl Default for TranslationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            glossary: Vec::new(),
            engine: EngineChoice::default(),
            llm_backend: Backend::default(),
            llm_threads: 0,
        }
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
    pub source: &'static str,
    pub repo: &'static str,
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
    /// TranslateGemma's state (None without it, e.g. in tests).
    pub llm: Option<LlmStatus>,
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
    /// What produced the text: "translategemma", "opus-mt", "lexique"
    /// (Counter-Strike callouts) or "cache".
    #[serde(default)]
    pub engine: Option<String>,
}

pub trait Engine: Send {
    fn translate(&mut self, text: &str) -> Result<String, String>;
    /// Faster, lower quality (live lines): greedy decoding.
    fn translate_fast(&mut self, text: &str) -> Result<String, String> {
        self.translate(text)
    }
    /// Engine behind the last translation (shown with the line).
    fn engine_name(&self) -> &'static str {
        "opus-mt"
    }
}

pub trait EngineLoader: Send + Sync {
    fn installed(&self, dir: &Path, source: &str) -> bool;
    fn load(&self, dir: &Path, source: &str) -> Result<Box<dyn Engine>, String>;
    /// New settings; true when the loaded engines must be rebuilt.
    fn configure(&self, _config: &TranslationConfig) -> bool {
        false
    }
    /// "Rechercher les modèles" / after a download: try failed engines again.
    fn rescan(&self) {}
    fn llm_status(&self) -> Option<LlmStatus> {
        None
    }
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
    pairs: Mutex<HashMap<&'static str, Pair>>,
    queue: Mutex<VecDeque<Job>>,
    wake: Condvar,
    cache: Mutex<Cache>,
    /// Callouts of the map being played, kept as spoken (F-36).
    map_terms: Mutex<Vec<Term>>,
    generation: AtomicU64,
    /// Asks the worker to (re)load the installed models.
    reload: AtomicBool,
    /// Asks the worker to drop its engines first (engine choice changed).
    reset: AtomicBool,
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
        let pairs = models::CATALOG
            .iter()
            .map(|p| {
                (
                    p.source,
                    Pair {
                        state: PairState::Missing,
                        error: None,
                        load_ms: None,
                    },
                )
            })
            .collect();
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
            reset: AtomicBool::new(false),
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
        let (changed_glossary, changed_engine) = {
            let mut c = self.inner.config.lock().unwrap();
            let g = c.glossary != config.glossary;
            let e = c.engine != config.engine
                || c.llm_backend != config.llm_backend
                || c.llm_threads != config.llm_threads
                || c.enabled != config.enabled;
            *c = config.clone();
            (g, e)
        };
        let rebuild = self.inner.loader.configure(&config);
        if changed_glossary || changed_engine {
            *self.inner.cache.lock().unwrap() = Cache::default();
        }
        if rebuild || (changed_engine && config.engine != EngineChoice::Auto) {
            self.inner.reset.store(true, Ordering::SeqCst);
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
        self.inner.loader.rescan();
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

impl Drop for TranslationService {
    fn drop(&mut self) {
        self.inner.shutdown.store(true, Ordering::SeqCst);
        self.inner.wake.notify_all();
    }
}

impl Inner {
    fn status(&self) -> TranslationStatus {
        let pairs = self.pairs.lock().unwrap();
        TranslationStatus {
            config: self.config.lock().unwrap().clone(),
            pairs: models::CATALOG
                .iter()
                .map(|p| {
                    let s = &pairs[p.source];
                    PairStatus {
                        source: p.source,
                        repo: p.repo,
                        state: s.state,
                        error: s.error.clone(),
                        load_ms: s.load_ms,
                    }
                })
                .collect(),
            models_dir: self.models_dir.display().to_string(),
            queued: self.queue.lock().unwrap().len(),
            llm: self.loader.llm_status(),
        }
    }

    fn set_pair(
        &self,
        source: &'static str,
        state: PairState,
        error: Option<String>,
        load_ms: Option<u32>,
    ) {
        {
            let mut pairs = self.pairs.lock().unwrap();
            let p = pairs.get_mut(source).expect("catalog pair");
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
        engine: None,
    }
}

fn worker_loop(inner: &Inner) {
    crate::speech::cpu::lower_current_thread_priority();
    let mut engines: HashMap<&'static str, Box<dyn Engine>> = HashMap::new();
    let mut last_llm = llm_signature(inner);
    while !inner.shutdown.load(Ordering::SeqCst) {
        if inner.reset.swap(false, Ordering::SeqCst) {
            engines.clear();
        }
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
        // TranslateGemma's latency or state moved: refresh the engine card.
        let sig = llm_signature(inner);
        if sig != last_llm {
            last_llm = sig;
            inner.events.status(&inner.status());
        }
        match (&job.reply, result) {
            (Some(tx), Some(t)) => {
                let _ = tx.send(t);
            }
            (None, Some(t)) => inner.events.result(&t),
            _ => {}
        }
    }
}

/// What the engine card shows of TranslateGemma, to send it only on change.
fn llm_signature(inner: &Inner) -> Option<String> {
    inner.loader.llm_status().map(|s| {
        format!(
            "{:?}{:?}{:?}{:?}{:?}",
            s.state, s.backend, s.p50_ms, s.error, s.translations
        )
    })
}

fn load_installed(inner: &Inner, engines: &mut HashMap<&'static str, Box<dyn Engine>>) {
    for p in models::CATALOG {
        if engines.contains_key(p.source) {
            continue;
        }
        if !inner.loader.installed(&inner.models_dir, p.source) {
            inner.set_pair(p.source, PairState::Missing, None, None);
            continue;
        }
        inner.set_pair(p.source, PairState::Loading, None, None);
        let t = Instant::now();
        match inner.loader.load(&inner.models_dir, p.source) {
            Ok(e) => {
                engines.insert(p.source, e);
                inner.set_pair(
                    p.source,
                    PairState::Ready,
                    None,
                    Some(t.elapsed().as_millis() as u32),
                );
            }
            Err(e) => inner.set_pair(p.source, PairState::Error, Some(e), None),
        }
    }
}

/// Translates one job; `None` when it was purged (nothing is published).
fn run(
    inner: &Inner,
    engines: &mut HashMap<&'static str, Box<dyn Engine>>,
    job: &Job,
) -> Option<Translation> {
    let manual = job.reply.is_some();
    if !manual && job.generation != inner.generation.load(Ordering::SeqCst) {
        return None;
    }
    let queue_ms = job.queued.elapsed().as_millis() as u32;
    let fail = |why: &str| Some(failure(job.id, &job.lang, &job.text, why, queue_ms));
    let mut config = inner.config.lock().unwrap().clone();
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
    if job.lang == "fr" {
        // F-32: already French, shown as is.
        return Some(Translation {
            translated: Some(job.text.clone()),
            error: None,
            ..failure(job.id, &job.lang, &job.text, "", queue_ms)
        });
    }
    // Known Counter-Strike callouts: exact, instant, no model needed (F-36).
    if let Some(text) = callouts::translate(&job.lang, &job.text) {
        return Some(Translation {
            translated: Some(text),
            error: None,
            partial: job.partial,
            engine: Some("lexique".into()),
            ..failure(job.id, &job.lang, &job.text, "", queue_ms)
        });
    }
    let Some(spec) = models::pair(&job.lang) else {
        return fail("langue non gérée");
    };
    // A live line older than 2 s has been replaced on screen: skip it.
    if job.partial && job.queued.elapsed() > Duration::from_secs(2) {
        return None;
    }
    if !manual && job.queued.elapsed() > MAX_WAIT {
        return fail("moteur indisponible (délai dépassé)");
    }
    let Some(engine) = engines.get_mut(spec.source) else {
        if job.partial {
            return None; // the final line will say why
        }
        // Present but unreadable is not the same as absent.
        let broken = inner
            .pairs
            .lock()
            .unwrap()
            .get(spec.source)
            .is_some_and(|p| p.state == PairState::Error);
        if broken {
            return fail("modèle de traduction illisible (voir Audio & IA)");
        }
        return fail("modèle de traduction absent");
    };
    let key = (job.lang.clone(), cache_key(&job.text));
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
                    engine: Some(engine.engine_name().into()),
                    ..failure(job.id, &job.lang, &job.text, "", queue_ms)
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
            engine: Some("cache".into()),
            ..failure(job.id, &job.lang, &job.text, "", queue_ms)
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
                engine: Some(engine.engine_name().into()),
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

/// Real models from `<models dir>/opus-mt-<src>-fr`.
pub struct OpusLoader;

impl EngineLoader for OpusLoader {
    fn installed(&self, dir: &Path, source: &str) -> bool {
        models::pair(source).is_some_and(|p| models::missing_file(dir, p).is_none())
    }

    fn load(&self, dir: &Path, source: &str) -> Result<Box<dyn Engine>, String> {
        let p = models::pair(source).ok_or("langue non gérée")?;
        let m = super::marian::Marian::load(&models::pair_dir(dir, p), super::marian::Mode::Fast)?;
        Ok(Box::new(m))
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
        fn installed(&self, _: &Path, source: &str) -> bool {
            self.installed.contains(&source)
        }
        fn load(&self, _: &Path, _: &str) -> Result<Box<dyn Engine>, String> {
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
        let mut engines: HashMap<&'static str, Box<dyn Engine>> = HashMap::new();
        engines.insert("en", Box::new(Dash));
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
}
