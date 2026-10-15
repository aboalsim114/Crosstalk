//! What the translation service takes and reports (serialised for the UI,
//! camelCase).

use serde::{Deserialize, Serialize};

use super::super::glossary::Term;
use super::super::languages;

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
    languages::DEFAULT_TARGET.into()
}

fn default_sources() -> Vec<String> {
    languages::DEFAULT_SOURCES
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
        if !languages::is_target(&self.target) {
            self.target = default_target();
        }
        let mut out: Vec<String> = Vec::new();
        for l in self.sources {
            if languages::is_language(&l) && !out.contains(&l) {
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
            .filter(|s| **s != self.target)
            .cloned()
            .collect()
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationStatus {
    pub config: TranslationConfig,
    /// The LLM can be asked (connected, configured, key or local server).
    pub ready: bool,
    /// Why translations are unavailable, shown as is.
    pub problem: Option<String>,
    pub queued: usize,
}

/// Result for one line, matched to the transcript by `id`.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Translation {
    pub id: u64,
    pub lang: String,
    pub original: String,
    /// Text in the user's language, or `None` with `error` set: the original
    /// stays displayed with the reason (no invented translation).
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
    /// What produced `translated`: "llm", or "lexicon" (built-in
    /// Counter-Strike callouts); `None` when untranslated or already in the
    /// user's language.
    #[serde(default)]
    pub engine: Option<String>,
}
