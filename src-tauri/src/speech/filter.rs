//! Post-recognition checks: Whisper "hears" words in noise (shots, explosions,
//! music) and has well-known hallucinations learnt from subtitled videos. A
//! transcript is shown only if it passes every check here.

use super::types::{Lang, RejectReason};

/// CS2 vocabulary given to Whisper as initial prompt: it biases spelling
/// ("AWP", "плент") without forcing words that were not said.
pub const DEFAULT_GLOSSARY: &[&str] = &[
    "rush B",
    "mid",
    "smoke",
    "flash",
    "molly",
    "AWP",
    "eco",
    "force buy",
    "plant",
    "defuse",
    "rotate",
    "плент",
    "раш",
    "смок",
    "флешка",
    "мид",
    "эко",
    "ротейт",
    "дефьюз",
    "авп",
];

/// What Whisper returned for a segment.
#[derive(Debug, Clone, PartialEq)]
pub struct Recognition {
    pub text: String,
    pub lang: Lang,
    pub lang_prob: f32,
    /// Mean log-probability of the text tokens.
    pub avg_logprob: f32,
    /// Probability of the "no speech" token, from the first decoder step.
    pub no_speech_prob: f32,
}

/// Rejection thresholds, aligned with OpenAI's reference decoder where it has one.
pub const NO_SPEECH_MAX: f32 = 0.6;
pub const NO_SPEECH_LOGPROB: f32 = -0.7;
pub const LOGPROB_MIN: f32 = -1.0;
/// Fast speech is ~15 characters per second; more means invented text.
pub const MAX_CHARS_PER_S: f32 = 28.0;

/// Phrases Whisper produces on silence or noise (normalised: lower case,
/// letters and spaces only). Matched as the whole text.
const HALLUCINATIONS: &[&str] = &[
    "продолжение следует",
    "спасибо за просмотр",
    "спасибо за внимание",
    "подписывайтесь на канал",
    "ставьте лайки",
    "до новых встреч",
    "всем пока",
    "thank you",
    "thanks for watching",
    "thank you for watching",
    "thanks for watching and see you next time",
    "please subscribe",
    "subscribe to my channel",
    "like and subscribe",
    "see you next time",
    "bye",
    "you",
    "music",
    "applause",
    "laughter",
    "the end",
    "музыка",
    "аплодисменты",
    "смех",
];

/// Fragments of subtitle credits that Whisper inserts anywhere.
const CREDIT_FRAGMENTS: &[&str] = &[
    "субтитры",
    "редактор субтитров",
    "корректор",
    "dimatorzok",
    "amara org",
    "subtitles by",
    "transcribed by",
    "translated by",
    "www ",
    "http",
];

/// Cleans the raw text: bracketed annotations, music notes, extra spaces.
pub fn clean(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut depth = 0usize;
    for c in text.chars() {
        match c {
            '[' | '(' | '{' | '<' | '*' => {
                if c == '*' && depth > 0 {
                    depth -= 1;
                } else {
                    depth += 1;
                }
            }
            ']' | ')' | '}' | '>' => depth = depth.saturating_sub(1),
            '♪' | '♫' | '♬' => {}
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    let joined = out.split_whitespace().collect::<Vec<_>>().join(" ");
    joined
        .trim_matches(|c: char| c == '-' || c == '—' || c == '–' || c.is_whitespace())
        .to_string()
}

fn normalise(text: &str) -> String {
    text.chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c.to_lowercase().next().unwrap_or(c)
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn is_cyrillic(c: char) -> bool {
    matches!(c, '\u{0400}'..='\u{04FF}')
}

/// Accepts a recognition (returns the cleaned text) or says why it is dropped.
pub fn judge(r: &Recognition, audio_ms: u32) -> Result<String, RejectReason> {
    let text = clean(&r.text);
    let letters: Vec<char> = text.chars().filter(|c| c.is_alphabetic()).collect();
    if letters.is_empty() {
        return Err(RejectReason::Empty);
    }
    if r.no_speech_prob > NO_SPEECH_MAX && r.avg_logprob < NO_SPEECH_LOGPROB {
        return Err(RejectReason::NoSpeech);
    }
    if r.avg_logprob < LOGPROB_MIN {
        return Err(RejectReason::LowConfidence);
    }

    let norm = normalise(&text);
    if HALLUCINATIONS.contains(&norm.as_str()) || CREDIT_FRAGMENTS.iter().any(|f| norm.contains(f))
    {
        return Err(RejectReason::Hallucination);
    }
    if is_repetitive(&norm) {
        return Err(RejectReason::Repetition);
    }
    let secs = (audio_ms.max(1) as f32) / 1000.0;
    if text.chars().count() as f32 / secs > MAX_CHARS_PER_S && text.chars().count() > 12 {
        return Err(RejectReason::Hallucination);
    }

    let cyrillic = letters.iter().filter(|c| is_cyrillic(**c)).count();
    // CS2 terms in Latin letters are fine inside Russian ("плент на B").
    let wrong_script = if r.lang.cyrillic() {
        cyrillic == 0
    } else {
        cyrillic * 2 > letters.len()
    };
    if wrong_script {
        return Err(RejectReason::WrongScript);
    }
    Ok(text)
}

/// Looping output: a word or short phrase repeated over and over.
fn is_repetitive(norm: &str) -> bool {
    let words: Vec<&str> = norm.split(' ').filter(|w| !w.is_empty()).collect();
    if words.len() >= 8 {
        let mut unique = words.clone();
        unique.sort_unstable();
        unique.dedup();
        if (unique.len() as f32) / (words.len() as f32) < 0.35 {
            return true;
        }
    }
    // Same n-gram (n = 1..3) four times in a row.
    for n in 1..=3 {
        if words.len() < n * 4 {
            continue;
        }
        let mut run = 1;
        let mut i = n;
        while i + n <= words.len() {
            if words[i..i + n] == words[i - n..i] {
                run += 1;
                if run >= 4 {
                    return true;
                }
                i += n;
            } else {
                run = 1;
                i += 1;
            }
        }
    }
    // A single character stretched out ("аааааааа").
    norm.split(' ').any(|w| {
        let mut chars = w.chars();
        let first = chars.next();
        w.chars().count() >= 8 && first.is_some_and(|f| chars.all(|c| c == f))
    })
}

/// Initial prompt from the glossary, kept short (Whisper uses at most 224 tokens).
pub fn prompt_from(glossary: &[String]) -> String {
    prompt_with(glossary, &[])
}

/// Glossary, then the callouts of the map being played: the map's callouts
/// always fit (at the end, nearest to the speech), the glossary fills the rest.
pub fn prompt_with(glossary: &[String], map: &[String]) -> String {
    const BUDGET: usize = 400;
    let map_len: usize = map.iter().map(|t| t.len() + 2).sum();
    let mut out = String::new();
    for term in glossary {
        if out.len() + term.len() + 2 + map_len > BUDGET {
            break;
        }
        if !out.is_empty() {
            out.push_str(", ");
        }
        out.push_str(term);
    }
    for term in map {
        if out.len() + term.len() + 2 > BUDGET {
            break;
        }
        if !out.is_empty() {
            out.push_str(", ");
        }
        out.push_str(term);
    }
    if !out.is_empty() {
        out.push('.');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(text: &str, lang: Lang) -> Recognition {
        Recognition {
            text: text.into(),
            lang,
            lang_prob: 0.9,
            avg_logprob: -0.3,
            no_speech_prob: 0.05,
        }
    }

    #[test]
    fn accepts_real_callouts() {
        assert_eq!(
            judge(&rec(" Двое идут на B, двое на B.", Lang::Ru), 2000),
            Ok("Двое идут на B, двое на B.".into())
        );
        assert_eq!(
            judge(&rec("one low, he's lit, eighty HP", Lang::En), 1800),
            Ok("one low, he's lit, eighty HP".into())
        );
        // Russian with CS terms in Latin letters.
        assert!(judge(&rec("плент B, AWP мид", Lang::Ru), 1500).is_ok());
    }

    #[test]
    fn cleans_annotations_and_dashes() {
        assert_eq!(clean("- [музыка] Бомба на A ♪"), "Бомба на A");
        assert_eq!(clean("(gunshot) rotate B *laughs*"), "rotate B");
        assert_eq!(clean("  a   b  "), "a b");
    }

    #[test]
    fn rejects_noise_only_and_low_confidence() {
        assert_eq!(
            judge(&rec("[Музыка]", Lang::Ru), 1000),
            Err(RejectReason::Empty)
        );
        assert_eq!(judge(&rec("...", Lang::En), 1000), Err(RejectReason::Empty));
        let mut r = rec("Бомба", Lang::Ru);
        r.no_speech_prob = 0.8;
        r.avg_logprob = -0.9;
        assert_eq!(judge(&r, 1000), Err(RejectReason::NoSpeech));
        r.no_speech_prob = 0.8;
        r.avg_logprob = -0.2;
        assert!(
            judge(&r, 1000).is_ok(),
            "confident text survives a high no-speech prob"
        );
        r.no_speech_prob = 0.1;
        r.avg_logprob = -1.4;
        assert_eq!(judge(&r, 1000), Err(RejectReason::LowConfidence));
    }

    #[test]
    fn rejects_known_hallucinations() {
        for t in [
            "Продолжение следует...",
            "Thanks for watching!",
            "Thank you.",
            "you",
            "Субтитры сделал DimaTorzok",
            "Редактор субтитров А.Синецкая Корректор А.Егорова",
        ] {
            let lang = if t.chars().any(is_cyrillic) {
                Lang::Ru
            } else {
                Lang::En
            };
            assert_eq!(
                judge(&rec(t, lang), 2000),
                Err(RejectReason::Hallucination),
                "{t}"
            );
        }
        // Too much text for the audio duration.
        let long = "раш бэ раш бэ быстрее все на бэ давайте ребята идём";
        assert_eq!(
            judge(&rec(long, Lang::Ru), 600),
            Err(RejectReason::Hallucination)
        );
    }

    #[test]
    fn rejects_loops() {
        assert_eq!(
            judge(&rec("go go go go go go", Lang::En), 3000),
            Err(RejectReason::Repetition)
        );
        assert_eq!(
            judge(&rec("на бэ на бэ на бэ на бэ", Lang::Ru), 4000),
            Err(RejectReason::Repetition)
        );
        assert_eq!(
            judge(&rec("ааааааааааа", Lang::Ru), 2000),
            Err(RejectReason::Repetition)
        );
        assert!(judge(&rec("go go go, push B", Lang::En), 2000).is_ok());
    }

    #[test]
    fn rejects_text_in_the_wrong_script() {
        assert_eq!(
            judge(&rec("rotate to B", Lang::Ru), 1500),
            Err(RejectReason::WrongScript)
        );
        assert_eq!(
            judge(&rec("ротейт на бэ", Lang::En), 1500),
            Err(RejectReason::WrongScript)
        );
        // Ukrainian is Cyrillic, German / Spanish / Polish / Turkish are Latin.
        assert!(judge(&rec("Двоє на Б, у них снайперка", Lang::Uk), 1800).is_ok());
        assert!(judge(&rec("zwei auf B, sie haben eine AWP", Lang::De), 1800).is_ok());
        assert!(judge(&rec("czekaj, idziemy na B", Lang::Pl), 1800).is_ok());
        assert_eq!(
            judge(&rec("двое на бэ", Lang::Es), 1500),
            Err(RejectReason::WrongScript)
        );
        assert_eq!(
            judge(&rec("push B now", Lang::Uk), 1500),
            Err(RejectReason::WrongScript)
        );
    }

    #[test]
    fn builds_a_bounded_prompt() {
        let g: Vec<String> = DEFAULT_GLOSSARY.iter().map(|s| s.to_string()).collect();
        let p = prompt_from(&g);
        assert!(p.starts_with("rush B, mid"));
        assert!(p.ends_with('.'));
        assert!(p.len() <= 402);
        assert_eq!(prompt_from(&[]), "");
        let many: Vec<String> = (0..200).map(|i| format!("term{i}")).collect();
        assert!(prompt_from(&many).len() <= 402);
    }

    #[test]
    fn map_callouts_always_fit_at_the_end() {
        let many: Vec<String> = (0..200).map(|i| format!("term{i}")).collect();
        let map: Vec<String> = ["Palace", "Jungle", "Connector"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let p = prompt_with(&many, &map);
        assert!(p.starts_with("term0, "));
        assert!(p.ends_with("Palace, Jungle, Connector."), "{p}");
        assert!(p.len() <= 402);
        assert_eq!(prompt_with(&[], &map), "Palace, Jungle, Connector.");
    }
}
