//! Translation engines (DeepL, Azure, local). API keys are stored in the Windows
//! Credential Manager, never in config files or logs.

use super::PipelineError;

pub trait Translator: Send + Sync {
    fn name(&self) -> &'static str;
    fn translate(&self, text: &str, from: &str, to: &str) -> Result<String, PipelineError>;
}

/// Lines already in the target language are shown as-is, without an API call.
pub fn needs_translation(source_lang: &str, target_lang: &str, skip_target: bool) -> bool {
    !(skip_target && source_lang.eq_ignore_ascii_case(target_lang))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skips_lines_already_in_french_when_enabled() {
        assert!(!needs_translation("fr", "fr", true));
        assert!(needs_translation("fr", "fr", false));
        assert!(needs_translation("ru", "fr", true));
    }
}
