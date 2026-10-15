//! What the user chose: the API format, the server address and the model.
//! The key is never part of it (see keys.rs).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    /// OpenAI and every API that copies it: OpenRouter, Mistral, Groq, Ollama
    /// (local or Cloud), LM Studio…
    #[default]
    Openai,
    Anthropic,
}

impl Provider {
    pub fn default_base_url(self) -> &'static str {
        match self {
            Provider::Openai => "https://api.openai.com/v1",
            Provider::Anthropic => "https://api.anthropic.com/v1",
        }
    }
}

/// Cloud (text sent to a provider over the Internet) or local (a server on
/// this PC or the local network: nothing leaves it).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Cloud,
    Local,
}

/// Persisted by the UI (`config.json`, without the key).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmConfig {
    /// The engine is connected (the user can disconnect without losing it).
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub provider: Provider,
    #[serde(default)]
    pub base_url: String,
    #[serde(default)]
    pub model: String,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: Provider::Openai,
            base_url: Provider::Openai.default_base_url().into(),
            model: String::new(),
        }
    }
}

impl LlmConfig {
    /// Trimmed, without a trailing slash; an empty address takes the provider's.
    pub fn sanitized(mut self) -> Self {
        self.base_url = self.base_url.trim().trim_end_matches('/').to_string();
        if self.base_url.is_empty() {
            self.base_url = self.provider.default_base_url().into();
        }
        self.model = self.model.trim().to_string();
        self
    }

    /// Why the configuration cannot be used, if it cannot.
    pub fn problem(&self) -> Option<String> {
        if let Some(p) = url_problem(&self.base_url) {
            return Some(p);
        }
        if self.model.is_empty() {
            return Some("modèle non choisi".into());
        }
        None
    }

    pub fn is_local(&self) -> bool {
        is_local(&self.base_url.to_ascii_lowercase())
    }

    pub fn kind(&self) -> Kind {
        if self.is_local() {
            Kind::Local
        } else {
            Kind::Cloud
        }
    }

    /// The local server, when its usual port tells it.
    pub fn backend(&self) -> Option<&'static str> {
        if !self.is_local() {
            return None;
        }
        let url = self.base_url.to_ascii_lowercase();
        if url.contains(":11434") {
            Some("Ollama")
        } else if url.contains(":1234") {
            Some("LM Studio")
        } else {
            None
        }
    }
}

/// The address is usable: https, or http only towards this PC or the local
/// network (a key must never travel in clear text over the Internet), and no
/// credentials hidden in it.
pub fn url_problem(url: &str) -> Option<String> {
    let lower = url.trim().to_ascii_lowercase();
    let rest = if let Some(r) = lower.strip_prefix("https://") {
        r
    } else if let Some(r) = lower.strip_prefix("http://") {
        if !is_local(&lower) {
            return Some("adresse http:// acceptée seulement en local (utilisez https://)".into());
        }
        r
    } else {
        return Some("adresse invalide (https://…)".into());
    };
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    if host.is_empty() {
        return Some("adresse invalide (https://…)".into());
    }
    if host.contains('@') {
        return Some("adresse invalide (identifiants dans l'adresse)".into());
    }
    if rest.contains(char::is_whitespace) {
        return Some("adresse invalide (espaces)".into());
    }
    None
}

/// `http(s)://localhost…`, loopback or private network addresses.
pub fn is_local(url: &str) -> bool {
    let rest = url
        .strip_prefix("http://")
        .or_else(|| url.strip_prefix("https://"))
        .unwrap_or("");
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host = if let Some(v6) = host.strip_prefix('[') {
        v6.split(']').next().unwrap_or("")
    } else {
        host.rsplit_once(':').map_or(host, |(h, _)| h)
    };
    if host == "localhost" || host == "::1" {
        return true;
    }
    match host.parse::<std::net::Ipv4Addr>() {
        Ok(ip) => ip.is_loopback() || ip.is_private() || ip.is_link_local(),
        Err(_) => false,
    }
}
