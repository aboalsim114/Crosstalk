// Object-safe asynchronous translation seam; implemented independently of speech.
#[allow(unused_imports)] // Consumed when the session pipeline is connected (#11).
pub use crate::translation::Translator;
/// French messages skip provider calls without affecting the original.
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
