//! The translation contract (docs/architecture/modules.md, "Contrats des
//! moteurs d'IA"). The service, "Répondre" and the dictation only know this
//! trait: the engine is the LLM the user chose (llm/), replaceable without
//! touching them.

pub trait Translator: Send + Sync {
    /// `None` when translations can be asked, else why not (shown as is).
    fn problem(&self) -> Option<String>;
    /// A final line.
    fn translate(&self, text: &str, source: &str, target: &str) -> Result<String, String>;
    /// A live line still being spoken; may refuse (cost, rate) without harm.
    fn translate_partial(&self, text: &str, source: &str, target: &str) -> Result<String, String>;
    /// Reported with each line ("llm").
    fn name(&self) -> &'static str {
        "llm"
    }
}
