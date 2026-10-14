//! Language of a short chat message (F-31): one of the languages Crosstalk
//! can translate (fr, en, ru, de, es, uk, pl, tr), or another ISO code for
//! scripts it cannot. Whisper gives the language of voices; chat messages are
//! a few words, so the guess relies on the alphabet, letters only one
//! language uses and frequent words. When nothing tells, the message is shown
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

const DE_WORDS: &[&str] = &[
    "der", "die", "das", "und", "ist", "ich", "du", "wir", "ihr", "sie", "nicht", "ein", "eine",
    "mit", "auf", "zu", "ja", "nein", "bitte", "danke", "gut", "schlecht", "hier", "da", "links",
    "rechts", "zwei", "drei", "warte", "wartet", "geht", "gehen", "kommt", "los", "habe", "hat",
    "sind", "noch", "auch", "was", "wo", "wie", "mal", "schon", "jetzt", "alle", "keine",
];

const ES_WORDS: &[&str] = &[
    "el",
    "los",
    "las",
    "y",
    "es",
    "yo",
    "tú",
    "nosotros",
    "que",
    "por",
    "para",
    "con",
    "una",
    "uno",
    "sí",
    "si",
    "no",
    "gracias",
    "bien",
    "mal",
    "aquí",
    "ahí",
    "izquierda",
    "derecha",
    "dos",
    "tres",
    "espera",
    "esperad",
    "vamos",
    "van",
    "tengo",
    "tiene",
    "están",
    "está",
    "pero",
    "muy",
    "todos",
    "nada",
    "ya",
    "dónde",
    "como",
    "cómo",
    "equipo",
    "jugar",
];

const PL_WORDS: &[&str] = &[
    "nie", "tak", "jest", "się", "na", "w", "z", "i", "to", "ja", "ty", "my", "oni", "dwa", "trzy",
    "czekaj", "idziemy", "idą", "tutaj", "tam", "lewo", "prawo", "dzięki", "proszę", "dobrze",
    "źle", "co", "gdzie", "jak", "już", "teraz", "wszyscy", "mam", "ma",
];

const TR_WORDS: &[&str] = &[
    "ve",
    "bir",
    "bu",
    "ne",
    "evet",
    "hayır",
    "ben",
    "sen",
    "biz",
    "onlar",
    "iki",
    "üç",
    "bekle",
    "gidiyoruz",
    "geliyor",
    "burada",
    "orada",
    "sol",
    "sağ",
    "teşekkürler",
    "sağol",
    "iyi",
    "kötü",
    "nerede",
    "nasıl",
    "şimdi",
    "hepsi",
    "var",
    "yok",
    "abi",
    "kanka",
];

/// Letters only one of the Latin-script languages uses.
fn marks(lang: &str) -> &'static str {
    match lang {
        "fr" => "éèêëàâçùûôîïœ",
        "de" => "äöüß",
        "es" => "ñáíóú¿¡",
        "pl" => "ąćęłńśźż",
        "tr" => "ğışİ",
        _ => "",
    }
}

/// Ukrainian letters absent from Russian.
const UK_LETTERS: &str = "іїєґІЇЄҐ";
/// Russian letters absent from Ukrainian.
const RU_LETTERS: &str = "ыэъЫЭЪ";

/// `fr`, `en`, `ru`, `de`, `es`, `uk`, `pl`, `tr`, or another ISO code for
/// scripts Crosstalk cannot translate; `None` when the message gives no clue
/// (`gg`, `?`, `rush b`).
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
        let uk = text.chars().filter(|c| UK_LETTERS.contains(*c)).count();
        let ru = text.chars().filter(|c| RU_LETTERS.contains(*c)).count();
        return Some(if uk > ru { "uk" } else { "ru" });
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
    let words: Vec<&str> = lower
        .split(|c: char| !(c.is_alphanumeric() || c == '\''))
        .filter(|w| !w.is_empty())
        .collect();
    let score = |lang: &str, list: &[&str]| {
        let w = words.iter().filter(|w| list.contains(w)).count();
        let m = text.chars().filter(|c| marks(lang).contains(*c)).count();
        w + 2 * m
    };
    let scores = [
        ("en", score("en", EN_WORDS)),
        ("fr", score("fr", FR_WORDS)),
        ("de", score("de", DE_WORDS)),
        ("es", score("es", ES_WORDS)),
        ("pl", score("pl", PL_WORDS)),
        ("tr", score("tr", TR_WORDS)),
    ];
    let best = scores.iter().map(|s| s.1).max().unwrap_or(0);
    let leaders: Vec<&str> = scores.iter().filter(|s| s.1 == best).map(|s| s.0).collect();
    // A tie or no clue at all: better shown as is than translated wrongly.
    (best > 0 && leaders.len() == 1).then(|| leaders[0])
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
    fn tells_the_other_supported_languages() {
        assert_eq!(detect("wartet, ich gehe links"), Some("de"));
        assert_eq!(detect("schöne Grüße"), Some("de"));
        assert_eq!(detect("espera, vamos por la derecha"), Some("es"));
        assert_eq!(detect("¿dónde están?"), Some("es"));
        assert_eq!(detect("czekaj, idziemy na lewo"), Some("pl"));
        assert_eq!(detect("dzięki"), Some("pl"));
        assert_eq!(detect("bekle, sağ taraftan gidiyoruz"), Some("tr"));
        assert_eq!(detect("Двоє на Б, у них снайперка, їх троє"), Some("uk"));
        assert_eq!(detect("Двое на Б, ещё трое"), Some("ru"));
    }

    #[test]
    fn says_nothing_when_nothing_tells() {
        for t in ["gg", "rush b", "?", "1v3", "ez", "nt", ":)"] {
            assert_eq!(detect(t), None, "{t}");
        }
    }
}
