//! Languages of the app: what the players speak (speech recognition) and what the user
//! reads. With an LLM, every pair translates directly: no model per pair, no
//! route through English.

/// Spoken or written by players (detected in each line) and readable by the
/// user. Turkish, Arabic, Chinese and Japanese come from the written chat
/// only: the speech recogniser does not write them (speech::types::Lang).
pub const LANGUAGES: [&str; 12] = [
    "fr", "en", "ru", "de", "es", "pt", "uk", "pl", "tr", "ar", "zh", "ja",
];
pub const DEFAULT_TARGET: &str = "fr";
pub const DEFAULT_SOURCES: [&str; 2] = ["ru", "en"];

pub fn is_language(code: &str) -> bool {
    LANGUAGES.contains(&code)
}

/// The user can read any of the languages.
pub fn is_target(code: &str) -> bool {
    is_language(code)
}

/// Languages "Répondre" can write into: the players' languages, except the user's own.
pub fn reply_targets(me: &str, sources: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for s in sources {
        if s != me && is_language(s) && !out.contains(s) {
            out.push(s.clone());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_pair_is_direct_and_replies_go_to_the_players_languages() {
        assert!(
            is_target("pl") && is_target("tr"),
            "the LLM writes every language"
        );
        // The 10 languages asked for, plus Ukrainian and Polish.
        for l in [
            "en", "fr", "ar", "ru", "zh", "es", "de", "pt", "tr", "ja", "uk", "pl",
        ] {
            assert!(is_target(l), "{l}");
        }
        assert!(!is_language("ko") && !is_language("it"));
        let players = vec![
            "ru".to_string(),
            "en".to_string(),
            "fr".to_string(),
            "ru".to_string(),
            "xx".to_string(),
        ];
        assert_eq!(reply_targets("fr", &players), vec!["ru", "en"]);
        assert_eq!(reply_targets("ru", &players), vec!["en", "fr"]);
    }
}
