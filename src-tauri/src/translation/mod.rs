//! Opt-in manual translation. No capture or speech source is connected here.
mod credentials;
mod glossary;
mod provider;
pub use credentials::{CredentialError, CredentialStore, WindowsCredentials};
pub use provider::{DeepL, ProviderError, TranslationRequest, Translator};
use serde::{Deserialize, Serialize};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::sync::{watch, Semaphore};
use zeroize::Zeroizing;
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TranslationPlan {
    #[default]
    Free,
    Pro,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TranslationSource {
    Ru,
    En,
    Fr,
}
impl TranslationSource {
    fn deepl(self) -> &'static str {
        match self {
            Self::Ru => "RU",
            Self::En => "EN",
            Self::Fr => "FR",
        }
    }
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GlossaryEntry {
    pub source: String,
    pub target: Option<String>,
}
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TranslationConfig {
    pub enabled: bool,
    pub plan: TranslationPlan,
    pub glossary: Vec<GlossaryEntry>,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationStatus {
    pub enabled: bool,
    pub plan: TranslationPlan,
    pub key_configured: bool,
    pub supported: bool,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationResult {
    pub original: String,
    pub translated: Option<String>,
    pub source: TranslationSource,
    pub target: &'static str,
    pub status: &'static str,
    pub reason: Option<String>,
    pub cached: bool,
    pub latency_ms: u32,
}
struct Cached {
    text: String,
    source: TranslationSource,
    translated: String,
}
#[derive(Default)]
struct State {
    config: TranslationConfig,
    epoch: u64,
    cache: VecDeque<Cached>,
}
pub struct TranslationService {
    state: Mutex<State>,
    credentials: Arc<dyn CredentialStore>,
    provider: Arc<dyn Translator>,
    slots: Semaphore,
    pending: Semaphore,
    epoch: watch::Sender<u64>,
}
impl TranslationService {
    pub fn new(provider: Arc<dyn Translator>, credentials: Arc<dyn CredentialStore>) -> Self {
        let (epoch, _) = watch::channel(0);
        Self {
            state: Mutex::new(State::default()),
            credentials,
            provider,
            slots: Semaphore::new(2),
            pending: Semaphore::new(16),
            epoch,
        }
    }
    pub fn native() -> Self {
        struct Unavailable;
        impl Translator for Unavailable {
            fn translate(
                &self,
                _: TranslationRequest,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = Result<String, ProviderError>> + Send + '_>,
            > {
                Box::pin(async { Err(ProviderError::Unavailable) })
            }
        }
        let provider: Arc<dyn Translator> = match DeepL::new() {
            Ok(provider) => Arc::new(provider),
            Err(_) => Arc::new(Unavailable),
        };
        Self::new(provider, Arc::new(WindowsCredentials))
    }
    fn status_locked(&self, state: &State) -> Result<TranslationStatus, String> {
        let supported = self.credentials.supported();
        let key_configured = supported
            && self
                .credentials
                .read(state.config.plan)
                .map_err(|_| "Gestionnaire d’identification indisponible.")?
                .is_some();
        Ok(TranslationStatus {
            enabled: state.config.enabled,
            plan: state.config.plan,
            key_configured,
            supported,
        })
    }
    pub fn status(&self) -> Result<TranslationStatus, String> {
        let state = self.state.lock().map_err(|_| "Moteur indisponible.")?;
        self.status_locked(&state)
    }
    fn invalidate(&self, state: &mut State) {
        state.epoch = state.epoch.wrapping_add(1);
        state.cache.clear();
        self.epoch.send_replace(state.epoch);
    }
    pub fn configure(&self, mut config: TranslationConfig) -> Result<TranslationStatus, String> {
        if config.glossary.len() > 256
            || config.glossary.iter().any(|e| {
                e.source.trim().is_empty()
                    || e.source.len() > 128
                    || e.target
                        .as_ref()
                        .is_some_and(|v| v.len() > 128 || v.trim().is_empty())
            })
        {
            return Err("Glossaire invalide (256 termes, 128 octets par terme maximum).".into());
        }
        let mut seen = std::collections::HashSet::new();
        if config
            .glossary
            .iter()
            .any(|e| !seen.insert(e.source.as_str()))
        {
            return Err("Le glossaire contient des termes en double.".into());
        }
        if config.enabled && !self.credentials.supported() {
            return Err("Traduction disponible uniquement sous Windows.".into());
        }
        let mut state = self.state.lock().map_err(|_| "Moteur indisponible.")?;
        // Consent is bound to the selected provider account; changing it requires
        // a separate explicit enable operation.
        if state.config.plan != config.plan {
            config.enabled = false;
        }
        if state.config != config {
            self.invalidate(&mut state);
            state.config = config;
        }
        self.status_locked(&state)
    }
    pub fn set_key(
        &self,
        plan: TranslationPlan,
        key: Zeroizing<String>,
    ) -> Result<TranslationStatus, String> {
        if key.is_empty()
            || key.len() > 4096
            || key.chars().any(|c| c.is_control() || c.is_whitespace())
        {
            return Err("Clé API invalide.".into());
        }
        let mut state = self.state.lock().map_err(|_| "Moteur indisponible.")?;
        self.credentials
            .write(plan, &key)
            .map_err(|_| "Impossible d’enregistrer la clé dans Windows.")?;
        state.config.enabled = false;
        self.invalidate(&mut state);
        self.status_locked(&state)
    }
    pub fn delete_key(&self, plan: TranslationPlan) -> Result<TranslationStatus, String> {
        let mut state = self.state.lock().map_err(|_| "Moteur indisponible.")?;
        self.credentials
            .delete(plan)
            .map_err(|_| "Impossible de supprimer la clé dans Windows.")?;
        state.config.enabled = false;
        self.invalidate(&mut state);
        self.status_locked(&state)
    }
    fn current(&self, epoch: u64) -> bool {
        self.state
            .lock()
            .is_ok_and(|s| s.epoch == epoch && s.config.enabled)
    }
    pub async fn translate(&self, text: String, source: TranslationSource) -> TranslationResult {
        let start = Instant::now();
        let mut result = TranslationResult {
            original: text.clone(),
            translated: None,
            source,
            target: "fr",
            status: "original",
            reason: None,
            cached: false,
            latency_ms: 0,
        };
        if text.trim().is_empty()
            || text.len() > 8192
            || text.chars().any(|c| {
                !matches!(c, '\t' | '\n' | '\r')
                    && (c < ' ' || matches!(c, '\u{fffe}' | '\u{ffff}'))
            })
        {
            result.reason = Some("Texte invalide (8192 octets maximum).".into());
            return result;
        }
        if source == TranslationSource::Fr {
            return result;
        }
        let _pending = match self.pending.try_acquire() {
            Ok(permit) => permit,
            Err(_) => {
                result.reason = Some("Moteur indisponible : trop de demandes.".into());
                return result;
            }
        };
        // Snapshot without holding a mutex across network awaits.
        let snapshot = (|| {
            let mut state = self.state.lock().map_err(|_| "Moteur indisponible.")?;
            if !state.config.enabled {
                return Err("Traduction désactivée.");
            }
            if !self.credentials.supported() {
                return Err("Traduction disponible uniquement sous Windows.");
            }
            let key = self
                .credentials
                .read(state.config.plan)
                .map_err(|_| "Gestionnaire d’identification indisponible.")?
                .ok_or("Clé DeepL absente.")?;
            if let Some(index) = state
                .cache
                .iter()
                .position(|c| c.text == text && c.source == source)
            {
                let cached = state.cache.remove(index).expect("found cache entry");
                result.translated = Some(cached.translated.clone());
                result.cached = true;
                result.status = "translated";
                state.cache.push_back(cached);
            }
            Ok((state.epoch, state.config.clone(), key))
        })();
        let (epoch, config, key) = match snapshot {
            Ok(v) => v,
            Err(reason) => {
                result.reason = Some(reason.into());
                return result;
            }
        };
        if result.cached {
            return result;
        }
        let mut changes = self.epoch.subscribe();
        let operation = async {
            let _permit = self
                .slots
                .acquire()
                .await
                .map_err(|_| ProviderError::Unavailable)?;
            let protected = glossary::Protected::new(&text, &config.glossary);
            if protected.xml.len() > 128 * 1024 {
                return Err(ProviderError::Invalid);
            }
            roxmltree::Document::parse(&protected.xml).map_err(|_| ProviderError::Invalid)?;
            for attempt in 0..2 {
                if !self.current(epoch) {
                    return Err(ProviderError::Unavailable);
                }
                let request = TranslationRequest {
                    xml: protected.xml.clone(),
                    source,
                    plan: config.plan,
                    key: key.clone(),
                };
                match tokio::time::timeout(
                    Duration::from_millis(2400),
                    self.provider.translate(request),
                )
                .await
                .unwrap_or(Err(ProviderError::Transient))
                {
                    Ok(xml) => {
                        let translated = protected
                            .restore(&xml)
                            .map_err(|_| ProviderError::Unavailable)?;
                        let mut state =
                            self.state.lock().map_err(|_| ProviderError::Unavailable)?;
                        if state.epoch != epoch || !state.config.enabled {
                            return Err(ProviderError::Unavailable);
                        }
                        if let Some(index) = state
                            .cache
                            .iter()
                            .position(|c| c.text == text && c.source == source)
                        {
                            state.cache.remove(index);
                        }
                        state.cache.push_back(Cached {
                            text: text.clone(),
                            source,
                            translated: translated.clone(),
                        });
                        if state.cache.len() > 1000 {
                            state.cache.pop_front();
                        }
                        return Ok(translated);
                    }
                    Err(ProviderError::Transient) if attempt == 0 => continue,
                    Err(error) => return Err(error),
                }
            }
            Err(ProviderError::Unavailable)
        };
        let deadline = tokio::time::Instant::from_std(start + Duration::from_secs(5));
        let outcome = tokio::time::timeout_at(deadline, async {
            if !self.current(epoch) { return Err(ProviderError::Unavailable); }
            tokio::select! { biased; _ = changes.changed() => Err(ProviderError::Unavailable), value = operation => value }
        }).await.unwrap_or(Err(ProviderError::Unavailable));
        match outcome {
            Ok(translated) if self.current(epoch) => {
                result.translated = Some(translated);
                result.status = "translated";
            }
            Ok(_) => result.reason = Some("Traduction annulée.".into()),
            Err(error) => result.reason = Some(error.message().into()),
        }
        result.latency_ms = start.elapsed().as_millis().min(u32::MAX as u128) as u32;
        result
    }
}
#[tauri::command]
pub fn translation_status(
    state: tauri::State<'_, Arc<TranslationService>>,
) -> Result<TranslationStatus, String> {
    state.status()
}
#[tauri::command]
pub fn translation_configure(
    config: TranslationConfig,
    state: tauri::State<'_, Arc<TranslationService>>,
) -> Result<TranslationStatus, String> {
    state.configure(config)
}
#[tauri::command]
pub fn translation_set_key(
    plan: TranslationPlan,
    key: String,
    state: tauri::State<'_, Arc<TranslationService>>,
) -> Result<TranslationStatus, String> {
    state.set_key(plan, Zeroizing::new(key))
}
#[tauri::command]
pub fn translation_delete_key(
    plan: TranslationPlan,
    state: tauri::State<'_, Arc<TranslationService>>,
) -> Result<TranslationStatus, String> {
    state.delete_key(plan)
}
#[tauri::command]
pub async fn translation_test(
    text: String,
    source: TranslationSource,
    state: tauri::State<'_, Arc<TranslationService>>,
) -> Result<TranslationResult, String> {
    Ok(state.translate(text, source).await)
}
#[cfg(test)]
mod tests;
