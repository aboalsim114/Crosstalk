//! Post-recognition checks: a recogniser can still "hear" words in noise
//! (shots, explosions, music), loop, or write in a script no allowed language
//! uses. A transcript is shown only if it passes every check here.

use super::types::{Lang, RejectReason};

/// What the recogniser returned for a segment.
#[derive(Debug, Clone, PartialEq)]
pub struct Recognition {
    pub text: String,
    pub lang: Lang,
    /// Confidence in `lang` (read from the text, see `engine::identify`).
    pub lang_prob: f32,
}

/// Fast speech is ~15 characters per second; more means invented text.
pub const MAX_CHARS_PER_S: f32 = 28.0;

/// Phrases speech recognisers produce on silence or noise (normalised: lower case,
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

/// Fragments of subtitle credits (seen in transcripts of noise).
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

/// A whole sentence followed by copies of itself ("Save your weapons this
/// round. Save your weapons this round. Save your…"): the model heard it once,
/// then looped. Returns that sentence. Only a phrase of at least three different
/// words repeated end to end counts: a chant ("go go go go") stays a loop.
pub fn collapse_repeats(text: &str) -> Option<String> {
    let words: Vec<&str> = text.split_whitespace().collect();
    let key = |w: &str| normalise(w);
    let last = words.len().saturating_sub(1);
    for period in 3..=words.len() / 2 {
        let reps = words.len() / period;
        // The output cap can cut the last copy mid-word ("weap…").
        let whole = words.iter().enumerate().skip(period).all(|(i, w)| {
            let (got, want) = (key(w), key(words[i % period]));
            got == want || (i == last && !got.is_empty() && want.starts_with(&got))
        });
        let mut distinct: Vec<String> = words[..period].iter().map(|w| key(w)).collect();
        distinct.sort_unstable();
        distinct.dedup();
        if reps >= 2 && whole && distinct.len() >= 3 {
            return Some(words[..period].join(" "));
        }
    }
    None
}

/// Accepts a recognition (returns the cleaned text) or says why it is dropped.
pub fn judge(r: &Recognition, audio_ms: u32) -> Result<String, RejectReason> {
    let text = clean(&r.text);
    let text = collapse_repeats(&text).unwrap_or(text);
    let letters: Vec<char> = text.chars().filter(|c| c.is_alphabetic()).collect();
    if letters.is_empty() {
        return Err(RejectReason::Empty);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(text: &str, lang: Lang) -> Recognition {
        Recognition {
            text: text.into(),
            lang,
            lang_prob: 0.9,
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
    fn rejects_noise_only() {
        assert_eq!(
            judge(&rec("[Музыка]", Lang::Ru), 1000),
            Err(RejectReason::Empty)
        );
        assert_eq!(judge(&rec("...", Lang::En), 1000), Err(RejectReason::Empty));
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
    fn keeps_a_sentence_repeated_after_hearing_it_once() {
        // Seen with the former recogniser (speaker loopback).
        assert_eq!(
            judge(
                &rec(
                    "Save your weapons this round. Save your weapons this round. Save your weapons this round. Save your weap",
                    Lang::En
                ),
                2080
            ),
            Ok("Save your weapons this round.".into())
        );
        assert_eq!(
            judge(
                &rec(
                    "Je suis presque mort, je recule. Je suis presque mort, je recule. Je suis presque mort, je recule.",
                    Lang::Fr
                ),
                2880
            ),
            Ok("Je suis presque mort, je recule.".into())
        );
        // Not a repeated sentence: unchanged, or still a loop.
        assert_eq!(collapse_repeats("plant on B, plant on A now"), None);
        assert_eq!(collapse_repeats("go go go go go go"), None);
        // A repeated hallucination is still caught once collapsed.
        assert_eq!(
            judge(
                &rec(
                    "Thanks for watching! Thanks for watching! Thanks for watching!",
                    Lang::En
                ),
                3000
            ),
            Err(RejectReason::Hallucination)
        );
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
}
