//! Language of a short chat message (F-31), limited to what Crosstalk can
//! translate (ru, en) or must leave alone (fr). Whisper gives the language of
//! voices; chat messages are a few words, so the guess relies on the alphabet,
//! French accents and frequent words. When nothing tells, the message is shown
//! as is rather than risking a wrong translation.

const EN_WORDS: &[&str] = &[
    "the", "you", "is", "are", "and", "to", "a", "on", "in", "it", "i", "we", "they", "he", "my",
    "your", "go", "going", "gonna", "wait", "with", "for", "of", "no", "yes", "what", "why",
    "need", "have", "has", "one", "two", "three", "left", "right", "here", "there", "push", "save",
    "buy", "drop", "me", "him", "them", "this", "that", "dont", "don't", "can", "please", "pls",
    "plz", "nice", "good", "bad", "team", "guys", "lol", "omg", "sorry", "thanks", "thx", "come",
    "back", "stay", "watch", "behind", "low", "hp", "lit", "rotate", "fake", "eco", "force",
    "full",
];

const FR_WORDS: &[&str] = &[
    "le", "la", "les", "un", "une", "des", "et", "est", "je", "tu", "il", "on", "nous", "vous",
    "ils", "pas", "ne", "de", "du", "au", "aux", "en", "dans", "sur", "avec", "pour", "mais", "ou",
    "qui", "que", "quoi", "moi", "toi", "lui", "mon", "ton", "ma", "ta", "ici", "là", "bien",
    "merci", "oui", "non", "deux", "trois", "gauche", "droite", "attends", "venez", "allez",
    "joue", "joueur", "c'est", "j'ai", "t'es", "y'a", "faut", "trop", "bravo", "désolé", "dsl",
    "mdr", "ptn", "putain",
];

/// `ru`, `en`, `fr`, or another ISO code for scripts Crosstalk cannot
/// translate; `None` when the message gives no clue (`gg`, `?`, `rush b`).
pub fn detect(text: &str) -> Option<&'static str> {
    let (mut cyr, mut latin, mut other) = (0usize, 0usize, None);
    for c in text.chars().filter(|c| c.is_alphabetic()) {
        match c {
            '\u{0400}'..='\u{04FF}' => cyr += 1,
            c if c.is_ascii_alphabetic() || ('\u{00C0}'..='\u{024F}').contains(&c) => latin += 1,
            '\u{4E00}'..='\u{9FFF}' => other = Some("zh"),
            '\u{3040}'..='\u{30FF}' => other = Some("ja"),
            '\u{AC00}'..='\u{D7AF}' => other = Some("ko"),
            '\u{0600}'..='\u{06FF}' => other = Some("ar"),
            '\u{0370}'..='\u{03FF}' => other = Some("el"),
            _ => {}
        }
    }
    if cyr > 0 && cyr * 2 >= cyr + latin {
        return Some("ru");
    }
    if let Some(o) = other {
        if latin == 0 {
            return Some(o);
        }
    }
    if latin == 0 {
        return None;
    }
    let lower = text.to_lowercase();
    let accents = lower
        .chars()
        .filter(|c| "éèêëàâçùûôîïœ".contains(*c))
        .count();
    let words: Vec<&str> = lower
        .split(|c: char| !(c.is_alphanumeric() || c == '\''))
        .filter(|w| !w.is_empty())
        .collect();
    let en = words.iter().filter(|w| EN_WORDS.contains(w)).count();
    let fr = words.iter().filter(|w| FR_WORDS.contains(w)).count() + 2 * accents;
    match (en, fr) {
        (0, 0) => None,
        (e, f) if f > e => Some("fr"),
        (e, f) if e > f => Some("en"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tells_the_languages_crosstalk_handles() {
        assert_eq!(detect("Двое на B, у них AWP"), Some("ru"));
        assert_eq!(detect("бомба A"), Some("ru"));
        assert_eq!(detect("gonna smoke mid, wait for my flash"), Some("en"));
        assert_eq!(detect("they are going B"), Some("en"));
        assert_eq!(detect("je vais à B, attendez moi"), Some("fr"));
        assert_eq!(detect("bien joué les gars"), Some("fr"));
        assert_eq!(detect("你好"), Some("zh"));
    }

    #[test]
    fn says_nothing_when_nothing_tells() {
        for t in ["gg", "rush b", "?", "1v3", "ez", "nt", ":)"] {
            assert_eq!(detect(t), None, "{t}");
        }
    }
}
