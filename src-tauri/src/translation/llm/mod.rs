//! TranslateGemma 4B, run locally by llama.cpp in the `crosstalk-llm` helper
//! (src-llm), as the main translation engine; Opus-MT stays the fallback.
//!
//! - `catalog`: the GGUF model, its source, size, fingerprint, licence;
//! - `prompt`: TranslateGemma's official prompt format;
//! - `process`: the helper process (stdin/stdout, no network);
//! - `runtime`: one helper for the session, latency, slow / failure fallback;
//! - [`HybridLoader`]: what the translation service loads for each language.

pub mod catalog;
pub mod process;
pub mod prompt;
pub mod runtime;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::service::{Engine, EngineLoader, TranslationConfig};
pub use runtime::{Backend, EngineChoice, LlmRuntime, LlmState, LlmStatus, Settings};

pub const LLM_ENGINE: &str = "translategemma";
pub const OPUS_ENGINE: &str = "opus-mt";

/// For each source language: TranslateGemma when usable, Opus-MT otherwise
/// (and for the quick translation of live lines on the CPU).
pub struct HybridLoader {
    pub rt: Arc<LlmRuntime>,
    pub opus: Arc<dyn EngineLoader>,
}

impl HybridLoader {
    pub fn new(rt: Arc<LlmRuntime>, opus: Arc<dyn EngineLoader>) -> Self {
        Self { rt, opus }
    }
}

impl EngineLoader for HybridLoader {
    fn installed(&self, dir: &Path, source: &str) -> bool {
        (self.rt.usable() && prompt::language_name(source).is_some())
            || self.opus.installed(dir, source)
    }

    fn load(&self, dir: &Path, source: &str) -> Result<Box<dyn Engine>, String> {
        let llm = self.rt.usable() && self.rt.ensure_started().is_ok();
        // Opus-MT is loaded now only when it is the engine; as a fallback it
        // is loaded the first time it is needed (its memory is not wasted).
        let opus = if !llm {
            Some(self.opus.load(dir, source).map_err(|e| {
                self.rt
                    .status()
                    .error
                    .map_or(e.clone(), |l| format!("{l} ; Opus-MT : {e}"))
            })?)
        } else {
            None
        };
        Ok(Box::new(Hybrid {
            source: source.to_string(),
            rt: self.rt.clone(),
            opus,
            opus_loader: self.opus.clone(),
            dir: dir.to_path_buf(),
            last: if llm { LLM_ENGINE } else { OPUS_ENGINE },
        }))
    }

    fn configure(&self, cfg: &TranslationConfig) -> bool {
        self.rt.configure(Settings {
            choice: cfg.engine,
            backend: cfg.llm_backend,
            threads: cfg.llm_threads,
            enabled: cfg.enabled,
        })
    }

    fn rescan(&self) {
        self.rt.reset_failures();
    }

    fn llm_status(&self) -> Option<LlmStatus> {
        Some(self.rt.status())
    }
}

pub struct Hybrid {
    source: String,
    rt: Arc<LlmRuntime>,
    opus: Option<Box<dyn Engine>>,
    opus_loader: Arc<dyn EngineLoader>,
    dir: PathBuf,
    last: &'static str,
}

impl Hybrid {
    fn opus(&mut self) -> Result<&mut Box<dyn Engine>, String> {
        if self.opus.is_none() {
            if !self.opus_loader.installed(&self.dir, &self.source) {
                return Err("Opus-MT absent".into());
            }
            self.opus = Some(self.opus_loader.load(&self.dir, &self.source)?);
        }
        Ok(self.opus.as_mut().expect("loaded above"))
    }
}

impl Engine for Hybrid {
    fn translate(&mut self, text: &str) -> Result<String, String> {
        let mut why = None;
        if self.rt.usable() {
            match self.rt.translate(&self.source, "fr", text) {
                Ok(t) => {
                    self.last = LLM_ENGINE;
                    return Ok(t);
                }
                Err(e) => why = Some(e),
            }
        }
        let out = match self.opus() {
            Ok(o) => o.translate(text),
            Err(e) => return Err(why.unwrap_or(e)),
        };
        self.last = OPUS_ENGINE;
        out
    }

    /// Live (partial) lines are replaced a moment later: Opus-MT's greedy
    /// search when installed, TranslateGemma only when it runs on the GPU.
    fn translate_fast(&mut self, text: &str) -> Result<String, String> {
        if let Ok(o) = self.opus() {
            let out = o.translate_fast(text);
            self.last = OPUS_ENGINE;
            return out;
        }
        let on_gpu = self.rt.status().backend.is_some_and(|b| b != "cpu");
        if self.rt.usable() && on_gpu {
            self.last = LLM_ENGINE;
            return self.rt.translate(&self.source, "fr", text);
        }
        Err("pas de traduction provisoire sans Opus-MT sur le processeur".into())
    }

    fn engine_name(&self) -> &'static str {
        self.last
    }
}

#[cfg(test)]
mod tests {
    use super::runtime::tests::{installed_dir, runtime, FakeSpawner};
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    struct Opus {
        installed: bool,
        loads: Arc<AtomicU32>,
    }
    struct OpusEngine;
    impl Engine for OpusEngine {
        fn translate(&mut self, text: &str) -> Result<String, String> {
            Ok(format!("OPUS({text})"))
        }
        fn translate_fast(&mut self, text: &str) -> Result<String, String> {
            Ok(format!("opus-fast({text})"))
        }
    }
    impl EngineLoader for Opus {
        fn installed(&self, _: &Path, _: &str) -> bool {
            self.installed
        }
        fn load(&self, _: &Path, _: &str) -> Result<Box<dyn Engine>, String> {
            if !self.installed {
                return Err("modèle absent".into());
            }
            self.loads.fetch_add(1, Ordering::SeqCst);
            Ok(Box::new(OpusEngine))
        }
    }

    fn loader(dir: &Path, sp: FakeSpawner, opus: bool) -> (HybridLoader, Arc<AtomicU32>) {
        let loads = Arc::new(AtomicU32::new(0));
        let l = HybridLoader::new(
            runtime(dir, sp),
            Arc::new(Opus {
                installed: opus,
                loads: loads.clone(),
            }),
        );
        (l, loads)
    }

    #[test]
    fn translategemma_first_opus_loaded_only_when_needed() {
        let dir = installed_dir("hyb1");
        let (l, loads) = loader(&dir, FakeSpawner::new(), true);
        assert!(l.installed(&dir, "ru"));
        let mut e = l.load(&dir, "ru").unwrap();
        assert_eq!(loads.load(Ordering::SeqCst), 0);
        assert_eq!(e.translate("два на Б").unwrap(), "FR(два на Б)");
        assert_eq!(e.engine_name(), LLM_ENGINE);
        // Live line on the CPU: Opus-MT, loaded on first use.
        assert_eq!(e.translate_fast("два").unwrap(), "opus-fast(два)");
        assert_eq!(e.engine_name(), OPUS_ENGINE);
        assert_eq!(loads.load(Ordering::SeqCst), 1);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn falls_back_to_opus_when_translategemma_cannot_start() {
        let dir = installed_dir("hyb2");
        let mut sp = FakeSpawner::new();
        sp.fail = true;
        let (l, _) = loader(&dir, sp, true);
        let mut e = l.load(&dir, "en").unwrap();
        assert_eq!(e.translate("rush B").unwrap(), "OPUS(rush B)");
        assert_eq!(e.engine_name(), OPUS_ENGINE);
        assert!(l.llm_status().unwrap().error.is_some());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn without_any_engine_the_error_says_why() {
        let dir = installed_dir("hyb3");
        let mut sp = FakeSpawner::new();
        sp.fail = true;
        let (l, _) = loader(&dir, sp, false);
        let e = l.load(&dir, "ru").err().unwrap();
        assert!(e.contains("impossible") && e.contains("Opus-MT"), "{e}");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn opus_choice_never_starts_the_helper() {
        let dir = installed_dir("hyb4");
        let sp = FakeSpawner::new();
        let spawns = sp.spawns.clone();
        let (l, _) = loader(&dir, sp, true);
        l.configure(&TranslationConfig {
            engine: EngineChoice::Opus,
            ..TranslationConfig::default()
        });
        let mut e = l.load(&dir, "ru").unwrap();
        assert_eq!(e.translate("да").unwrap(), "OPUS(да)");
        assert_eq!(spawns.load(Ordering::SeqCst), 0);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
