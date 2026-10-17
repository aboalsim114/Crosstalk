//! Translation by the LLM the user chose (Audio & IA › Traduction IA): the
//! only translation engine of Crosstalk.
//!
//! - **Cloud**: the user's provider and key. Any OpenAI-compatible API
//!   (`POST {base}/chat/completions`: OpenAI, Mistral, Groq, OpenRouter,
//!   Ollama Cloud…) or the Anthropic API (`POST {base}/messages`).
//! - **Local**: a server already installed on this PC (Ollama, LM Studio, or
//!   any OpenAI-compatible address on the local network), without a key.
//!
//! Only the text of the line is sent: never the audio, never a player's name.
//! The key lives in the Windows Credential Manager, never in the settings
//! file, the log, the status or an error message. There is no fallback
//! engine: when the LLM fails, the line stays in its language with the reason.
//!
//! Responsibilities: `config` (provider, address, model, local/cloud),
//! `keys` (Credential Manager), `transport` (HTTP, no redirects), `prompt`
//! (CS2 instructions, TranslateGemma format, request bodies), `validate`
//! (answers that are not a translation are rejected), `local` (Ollama and
//! LM Studio detection), `client` (calls, pauses, counters, tests).

pub mod client;
pub mod config;
pub mod keys;
pub mod local;
pub mod prompt;
pub mod transport;
pub mod validate;

pub use client::{
    LlmClient, LlmModels, LlmStats, LlmStatus, LlmTest, CLOUD_TIMEOUT, LOCAL_TIMEOUT,
};
pub use config::{Kind, LlmConfig, Provider};
pub use keys::{clean_key, default_key_store, KeyStore, MemoryKeyStore};
pub use local::{detect_local, detect_ollama, LocalServer, OllamaLocal};
pub use transport::{HttpError, Transport, UreqTransport};
pub use validate::plausible;

use super::engine::Translator;

/// Pages where each service creates an API key, opened from the app. A fixed
/// list: the interface names a service, never a URL.
pub const KEY_PAGES: [(&str, &str); 8] = [
    ("openai", "https://platform.openai.com/api-keys"),
    ("openrouter", "https://openrouter.ai/keys"),
    ("mistral", "https://console.mistral.ai/api-keys"),
    ("groq", "https://console.groq.com/keys"),
    ("anthropic", "https://console.anthropic.com/settings/keys"),
    ("ollama-cloud", "https://ollama.com/settings/keys"),
    ("ollama-download", "https://ollama.com/download"),
    ("lmstudio-download", "https://lmstudio.ai/"),
];

pub fn key_page(service: &str) -> Option<&'static str> {
    KEY_PAGES
        .iter()
        .find(|(s, _)| *s == service)
        .map(|(_, url)| *url)
}

/// Sentence translated by "Tester la connexion".
pub const TEST_SENTENCE: &str = "Двое идут на B, двое на B";

impl Translator for LlmClient {
    fn problem(&self) -> Option<String> {
        let st = self.status();
        if !st.config.enabled {
            return Some("aucun moteur de traduction connecté (Audio & IA › Traduction IA)".into());
        }
        st.problem
    }

    fn translate(&self, text: &str, source: &str, target: &str) -> Result<String, String> {
        LlmClient::translate(self, text, source, target)
    }

    fn translate_partial(&self, text: &str, source: &str, target: &str) -> Result<String, String> {
        LlmClient::translate_partial(self, text, source, target)
    }

    fn translate_strict(&self, text: &str, source: &str, target: &str) -> Result<String, String> {
        LlmClient::translate_strict(self, text, source, target)
    }
}

#[cfg(test)]
mod tests;
