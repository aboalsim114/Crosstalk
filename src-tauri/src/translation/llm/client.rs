//! The LLM client shared by every translation of the app (voice lines, chat,
//! "Répondre", dictated replies, tests): one configuration, one key, one set
//! of counters. Errors never contain the key, the request or the raw answer.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::Value;

use super::config::{Kind, LlmConfig};
use super::keys::{clean_key, KeyStore};
use super::prompt::{self, extract};
use super::transport::{describe, HttpError, Transport};
use super::validate::plausible;

/// A cloud call slower than this fails (the line stays in its language).
pub const CLOUD_TIMEOUT: Duration = Duration::from_secs(6);
/// A model on this PC may take longer, the first time especially.
pub const LOCAL_TIMEOUT: Duration = Duration::from_secs(15);
/// After a 429, no call for this long (no rain of paid, refused requests).
pub const QUOTA_PAUSE: Duration = Duration::from_secs(20);
/// Live (provisional) lines: local servers only, at most one per interval.
pub const PARTIAL_INTERVAL: Duration = Duration::from_millis(1500);

/// Counters shown in the UI.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmStats {
    pub calls: u64,
    pub failures: u64,
    /// Successful translations.
    pub translations: u64,
    pub last_latency_ms: Option<u32>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmStatus {
    pub config: LlmConfig,
    pub kind: Kind,
    /// "Ollama", "LM Studio" when the address tells it.
    pub backend: Option<&'static str>,
    /// A key is stored (its value never leaves the Rust core).
    pub has_key: bool,
    /// Lines are sent to the LLM.
    pub active: bool,
    /// Why it is not active although enabled (or paused).
    pub problem: Option<String>,
    /// Seconds before calls resume after a quota error.
    pub paused_s: Option<u32>,
    pub key_storage: String,
    #[serde(flatten)]
    pub stats: LlmStats,
}

/// Result of a connection or translation test.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LlmTest {
    pub ok: bool,
    pub ms: u32,
    pub original: String,
    pub translated: Option<String>,
    pub error: Option<String>,
}

/// Models offered by the configured server (`GET …/models`).
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LlmModels {
    pub models: Vec<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
enum Block {
    /// Quota or rate limit: calls resume at this time.
    Until(Instant),
    /// Key refused or no credit: no call until the settings or the key change.
    Refused(String),
}

pub struct LlmClient {
    config: Mutex<LlmConfig>,
    keys: Arc<dyn KeyStore>,
    transport: Box<dyn Transport>,
    stats: Mutex<LlmStats>,
    block: Mutex<Option<Block>>,
    last_partial: Mutex<Option<Instant>>,
}

/// Defence in depth: whatever happens, the key never reaches a message.
pub fn redact(message: &str, key: Option<&str>) -> String {
    match key {
        Some(k) if k.len() >= 4 && message.contains(k) => message.replace(k, "[clé masquée]"),
        _ => message.to_string(),
    }
}

impl LlmClient {
    pub fn new(keys: Arc<dyn KeyStore>, transport: Box<dyn Transport>) -> Self {
        Self {
            config: Mutex::new(LlmConfig::default()),
            keys,
            transport,
            stats: Mutex::new(LlmStats::default()),
            block: Mutex::new(None),
            last_partial: Mutex::new(None),
        }
    }

    pub fn config(&self) -> LlmConfig {
        self.config.lock().unwrap().clone()
    }

    pub fn configure(&self, config: LlmConfig) -> LlmStatus {
        let config = config.sanitized();
        // The settings lock is released before status() takes it again, also
        // when nothing changed (the case at every launch).
        let changed = {
            let mut c = self.config.lock().unwrap();
            let changed = *c != config;
            if changed {
                *c = config;
            }
            changed
        };
        if changed {
            // Counters and pauses of another provider or model would mislead.
            *self.stats.lock().unwrap() = LlmStats::default();
            *self.block.lock().unwrap() = None;
        }
        self.status()
    }

    pub fn set_key(&self, key: &str) -> Result<LlmStatus, String> {
        let key = clean_key(key)?;
        self.keys.set(&key)?;
        self.stats.lock().unwrap().last_error = None;
        *self.block.lock().unwrap() = None;
        Ok(self.status())
    }

    pub fn clear_key(&self) -> Result<LlmStatus, String> {
        self.keys.clear()?;
        Ok(self.status())
    }

    fn timeout(config: &LlmConfig) -> Duration {
        if config.is_local() {
            LOCAL_TIMEOUT
        } else {
            CLOUD_TIMEOUT
        }
    }

    fn problem(config: &LlmConfig, has_key: bool) -> Option<String> {
        config
            .problem()
            .or_else(|| (!has_key && !config.is_local()).then(|| "clé API non enregistrée".into()))
    }

    /// Why calls are held back right now (quota pause, refused key).
    fn blocked(&self) -> Option<(String, Option<u32>)> {
        let mut b = self.block.lock().unwrap();
        match b.clone() {
            Some(Block::Until(t)) if t > Instant::now() => {
                let s = (t - Instant::now()).as_secs() as u32 + 1;
                Some((
                    format!("quota ou limite de débit atteint : nouvel essai dans {s} s"),
                    Some(s),
                ))
            }
            Some(Block::Until(_)) => {
                *b = None;
                None
            }
            Some(Block::Refused(why)) => Some((
                format!("{why} : vérifiez la clé ou le compte, puis enregistrez-la à nouveau"),
                None,
            )),
            None => None,
        }
    }

    pub fn status(&self) -> LlmStatus {
        let config = self.config();
        let has_key = self.keys.get().is_some();
        let blocked = self.blocked();
        let problem =
            Self::problem(&config, has_key).or_else(|| blocked.as_ref().map(|b| b.0.clone()));
        LlmStatus {
            active: config.enabled && Self::problem(&config, has_key).is_none(),
            problem: config.enabled.then_some(problem).flatten(),
            paused_s: blocked.and_then(|b| b.1),
            kind: config.kind(),
            backend: config.backend(),
            config,
            has_key,
            key_storage: self.keys.label().into(),
            stats: self.stats.lock().unwrap().clone(),
        }
    }

    /// Lines go to the LLM (connected, configured, key present or local server).
    pub fn active(&self) -> bool {
        let config = self.config.lock().unwrap();
        config.enabled && Self::problem(&config, self.keys.get().is_some()).is_none()
    }

    /// Translates one line with the configured engine.
    pub fn translate(&self, text: &str, source: &str, target: &str) -> Result<String, String> {
        let config = self.config();
        let key = self.keys.get();
        if let Some(p) = Self::problem(&config, key.is_some()) {
            return Err(p);
        }
        if let Some((why, _)) = self.blocked() {
            return Err(why);
        }
        let started = Instant::now();
        let result = self.call(&config, key.as_deref(), text, source, target);
        let ms = started.elapsed().as_millis() as u32;
        let mut st = self.stats.lock().unwrap();
        st.calls += 1;
        match &result {
            Ok(_) => {
                st.translations += 1;
                st.last_latency_ms = Some(ms);
                st.last_error = None;
            }
            Err((e, _)) => {
                st.failures += 1;
                st.last_error = Some(redact(e, key.as_deref()));
            }
        }
        drop(st);
        match result {
            Ok(t) => Ok(t),
            Err((e, http)) => {
                match http {
                    Some(HttpError::Status(429)) => {
                        *self.block.lock().unwrap() =
                            Some(Block::Until(Instant::now() + QUOTA_PAUSE))
                    }
                    Some(HttpError::Status(401..=403)) => {
                        *self.block.lock().unwrap() = Some(Block::Refused(e.clone()))
                    }
                    _ => {}
                }
                Err(redact(&e, key.as_deref()))
            }
        }
    }

    /// A live line: only towards a local server, and not more than one per
    /// [`PARTIAL_INTERVAL`] (a cloud provider would bill every live update).
    pub fn translate_partial(
        &self,
        text: &str,
        source: &str,
        target: &str,
    ) -> Result<String, String> {
        if !self.config().is_local() {
            return Err("provisoire non envoyé (fournisseur cloud)".into());
        }
        {
            let mut last = self.last_partial.lock().unwrap();
            if last.is_some_and(|t| t.elapsed() < PARTIAL_INTERVAL) {
                return Err("provisoire ignoré (trop rapproché)".into());
            }
            *last = Some(Instant::now());
        }
        self.translate(text, source, target)
    }

    /// One request; the HTTP error comes back too, for the pauses.
    fn call(
        &self,
        config: &LlmConfig,
        key: Option<&str>,
        text: &str,
        source: &str,
        target: &str,
    ) -> Result<String, (String, Option<HttpError>)> {
        let (url, headers, body) = prompt::request(config, key, text, source, target);
        let timeout = Self::timeout(config);
        let raw = self
            .transport
            .post(&url, &headers, &body, timeout)
            .map_err(|e| (describe(&e, timeout), Some(e)))?;
        let answer: Value =
            serde_json::from_str(&raw).map_err(|_| ("réponse illisible".to_string(), None))?;
        let out =
            extract(config.provider, &answer).ok_or(("réponse illisible".to_string(), None))?;
        plausible(text, &out).map_err(|e| (e, None))
    }

    /// Translates `text` with the current settings, even while disconnected,
    /// so the setup can be checked before connecting. Not counted.
    pub fn test(&self, text: &str, source: &str, target: &str) -> LlmTest {
        let started = Instant::now();
        let config = self.config();
        let key = self.keys.get();
        let out = match Self::problem(&config, key.is_some()) {
            Some(p) => Err(p),
            None => self
                .call(&config, key.as_deref(), text, source, target)
                .map_err(|(e, _)| redact(&e, key.as_deref())),
        };
        if out.is_ok() {
            // The user fixed the setup: refused-key and quota pauses end.
            *self.block.lock().unwrap() = None;
        }
        LlmTest {
            ok: out.is_ok(),
            ms: started.elapsed().as_millis() as u32,
            original: text.into(),
            translated: out.as_ref().ok().cloned(),
            error: out.err(),
        }
    }

    /// Models of the configured server (cloud with the stored key, or local).
    pub fn list_models(&self) -> LlmModels {
        let config = self.config();
        if let Some(p) = super::config::url_problem(&config.base_url) {
            return LlmModels {
                models: Vec::new(),
                error: Some(p),
            };
        }
        let key = self.keys.get();
        if key.is_none() && !config.is_local() {
            return LlmModels {
                models: Vec::new(),
                error: Some("clé API non enregistrée".into()),
            };
        }
        let (url, headers) = prompt::models_request(&config, key.as_deref());
        match self.transport.get(&url, &headers, Duration::from_secs(8)) {
            Ok(body) => {
                let models = prompt::parse_models(&body);
                let error = models
                    .is_empty()
                    .then(|| "aucun modèle annoncé par le serveur : saisissez son nom".to_string());
                LlmModels { models, error }
            }
            Err(e) => LlmModels {
                models: Vec::new(),
                error: Some(redact(
                    &describe(&e, Duration::from_secs(8)),
                    key.as_deref(),
                )),
            },
        }
    }

    /// Local server: one tiny request in the background so that the model is
    /// loaded before the first real line (Ollama loads it on demand).
    pub fn warm_up(self: &Arc<Self>) {
        let config = self.config();
        if !config.enabled || !config.is_local() || config.problem().is_some() {
            return;
        }
        let me = self.clone();
        let _ = std::thread::Builder::new()
            .name("llm-warm-up".into())
            .spawn(move || {
                let (url, headers, body) = prompt::request(&config, None, "ok", "en", "fr");
                let _ = me
                    .transport
                    .post(&url, &headers, &body, Duration::from_secs(90));
            });
    }
}
