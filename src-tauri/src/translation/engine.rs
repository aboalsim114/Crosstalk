//! Contracts of the translation engines (docs/architecture/modules.md,
//! "Contrats des moteurs d'IA"). The service only knows these traits: a new
//! engine is a new `EngineLoader`, the service does not change.

use std::path::Path;

/// One loaded translation route (source → target).
pub trait Engine: Send {
    fn translate(&mut self, text: &str) -> Result<String, String>;
    /// Faster, lower quality (live lines): greedy decoding.
    fn translate_fast(&mut self, text: &str) -> Result<String, String> {
        self.translate(text)
    }
    /// Engine that produced the last successful translation ("llm",
    /// "opus-mt"), reported with each line.
    fn last_engine(&self) -> Option<&'static str> {
        None
    }
}

pub trait EngineLoader: Send + Sync {
    fn installed(&self, dir: &Path, source: &str, target: &str) -> bool;
    fn load(&self, dir: &Path, source: &str, target: &str) -> Result<Box<dyn Engine>, String>;
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
    fn last_engine(&self) -> Option<&'static str> {
        Some("opus-mt")
    }
}

impl Engine for super::marian::Marian {
    fn translate(&mut self, text: &str) -> Result<String, String> {
        super::marian::Marian::translate(self, text)
    }
    fn translate_fast(&mut self, text: &str) -> Result<String, String> {
        self.translate_with(text, 1)
    }
    fn last_engine(&self) -> Option<&'static str> {
        Some("opus-mt")
    }
}
