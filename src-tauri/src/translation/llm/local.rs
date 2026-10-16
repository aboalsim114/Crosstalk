//! LLM servers already installed on this PC. Crosstalk never installs or
//! downloads a model: it only asks the server what it has.
//!
//! - Ollama: `GET http://localhost:11434/api/tags` (installed models);
//! - LM Studio: `GET http://localhost:1234/v1/models` (downloaded models;
//!   LM Studio loads the chosen one on the first request).

use std::time::Duration;

use serde::Serialize;
use serde_json::Value;

use super::prompt::parse_models;
use super::transport::{Transport, UreqTransport};

pub const OLLAMA_LOCAL: &str = "http://localhost:11434";
pub const OLLAMA_API: &str = "http://localhost:11434/v1";
pub const LMSTUDIO_API: &str = "http://localhost:1234/v1";
/// A server on this PC answers at once.
const PROBE: Duration = Duration::from_millis(1500);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LocalKind {
    Ollama,
    Lmstudio,
}

/// One local server and its models.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LocalServer {
    pub kind: LocalKind,
    /// OpenAI-compatible address to use for translations.
    pub base_url: String,
    pub running: bool,
    pub models: Vec<String>,
}

/// Kept for the existing `llm_ollama_local` command.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OllamaLocal {
    pub running: bool,
    pub models: Vec<String>,
}

/// Names in Ollama's `GET /api/tags` answer.
pub fn parse_ollama_tags(json: &str) -> Vec<String> {
    let v: Value = serde_json::from_str(json).unwrap_or(Value::Null);
    let mut names: Vec<String> = v["models"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|m| m["name"].as_str().or_else(|| m["model"].as_str()))
                .map(String::from)
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names.dedup();
    names
}

pub fn probe_ollama(t: &dyn Transport) -> LocalServer {
    let r = t.get(&format!("{OLLAMA_LOCAL}/api/tags"), &[], PROBE);
    LocalServer {
        kind: LocalKind::Ollama,
        base_url: OLLAMA_API.into(),
        running: r.is_ok(),
        models: r.map(|b| parse_ollama_tags(&b)).unwrap_or_default(),
    }
}

pub fn probe_lmstudio(t: &dyn Transport) -> LocalServer {
    let r = t.get(&format!("{LMSTUDIO_API}/models"), &[], PROBE);
    LocalServer {
        kind: LocalKind::Lmstudio,
        base_url: LMSTUDIO_API.into(),
        running: r.is_ok(),
        // Embedding models cannot translate.
        models: r
            .map(|b| parse_models(&b))
            .unwrap_or_default()
            .into_iter()
            .filter(|m| !m.to_ascii_lowercase().contains("embed"))
            .collect(),
    }
}

/// Both servers, on the real network.
pub fn detect_local() -> Vec<LocalServer> {
    vec![probe_ollama(&UreqTransport), probe_lmstudio(&UreqTransport)]
}

pub fn detect_ollama() -> OllamaLocal {
    let s = probe_ollama(&UreqTransport);
    OllamaLocal {
        running: s.running,
        models: s.models,
    }
}
