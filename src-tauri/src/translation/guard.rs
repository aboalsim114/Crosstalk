//! Smart Comms safeguard: a translation must keep the tactical facts of the
//! line: how many (player counts, 2 to 10), where (sites A and B) and which
//! side (left / right). Checked on every LLM answer; a changed fact makes the
//! line go back once with a firmer prompt, then stay in its language.
//!
//! Only what can be told for sure is checked: "one" is not (it is also an
//! article: un, ein, uno…), numbers above 10 are not (health, money, time),
//! a capital "A" opening a sentence may be the English article. On doubt the
//! answer passes.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Issue {
    Number,
    Site,
    Direction,
}

impl Issue {
    /// Shown with the line kept in its language.
    pub fn reason(self) -> &'static str {
        match self {
            Issue::Number => "nombre modifié",
            Issue::Site => "position (A / B) modifiée",
            Issue::Direction => "direction (gauche / droite) modifiée",
        }
    }
}

/// Number words of each language, 1 to 10 ("both" counts as 2).
fn number_words(lang: &str) -> &'static [(&'static str, u8)] {
    match lang {
        "en" => &[
            ("one", 1),
            ("two", 2),
            ("both", 2),
            ("three", 3),
            ("four", 4),
            ("five", 5),
            ("six", 6),
            ("seven", 7),
            ("eight", 8),
            ("nine", 9),
            ("ten", 10),
        ],
        "fr" => &[
            ("un", 1),
            ("une", 1),
            ("deux", 2),
            ("trois", 3),
            ("quatre", 4),
            ("cinq", 5),
            ("six", 6),
            ("sept", 7),
            ("huit", 8),
            ("neuf", 9),
            ("dix", 10),
        ],
        "de" => &[
            ("ein", 1),
            ("eine", 1),
            ("eins", 1),
            ("zwei", 2),
            ("beide", 2),
            ("drei", 3),
            ("vier", 4),
            ("fünf", 5),
            ("sechs", 6),
            ("sieben", 7),
            ("acht", 8),
            ("neun", 9),
            ("zehn", 10),
        ],
        "es" => &[
            ("un", 1),
            ("uno", 1),
            ("una", 1),
            ("dos", 2),
            ("ambos", 2),
            ("tres", 3),
            ("cuatro", 4),
            ("cinco", 5),
            ("seis", 6),
            ("siete", 7),
            ("ocho", 8),
            ("nueve", 9),
            ("diez", 10),
        ],
        "pt" => &[
            ("um", 1),
            ("uma", 1),
            ("dois", 2),
            ("duas", 2),
            ("ambos", 2),
            ("três", 3),
            ("tres", 3),
            ("quatro", 4),
            ("cinco", 5),
            ("seis", 6),
            ("sete", 7),
            ("oito", 8),
            ("nove", 9),
            ("dez", 10),
        ],
        "ru" => &[
            ("один", 1),
            ("одна", 1),
            ("одного", 1),
            ("одну", 1),
            ("два", 2),
            ("две", 2),
            ("двое", 2),
            ("двух", 2),
            ("двоих", 2),
            ("оба", 2),
            ("обе", 2),
            ("три", 3),
            ("трое", 3),
            ("трёх", 3),
            ("трех", 3),
            ("троих", 3),
            ("четыре", 4),
            ("четверо", 4),
            ("четырёх", 4),
            ("четырех", 4),
            ("пять", 5),
            ("пятеро", 5),
            ("пяти", 5),
            ("шесть", 6),
            ("семь", 7),
            ("восемь", 8),
            ("девять", 9),
            ("десять", 10),
        ],
        "uk" => &[
            ("один", 1),
            ("одна", 1),
            ("два", 2),
            ("дві", 2),
            ("двоє", 2),
            ("двох", 2),
            ("обидва", 2),
            ("три", 3),
            ("троє", 3),
            ("трьох", 3),
            ("чотири", 4),
            ("четверо", 4),
            ("п'ять", 5),
            ("пʼять", 5),
            ("п'ятеро", 5),
            ("шість", 6),
            ("сім", 7),
            ("вісім", 8),
            ("дев'ять", 9),
            ("десять", 10),
        ],
        "pl" => &[
            ("jeden", 1),
            ("jedna", 1),
            ("dwa", 2),
            ("dwie", 2),
            ("dwóch", 2),
            ("dwaj", 2),
            ("obaj", 2),
            ("oba", 2),
            ("trzy", 3),
            ("trzech", 3),
            ("trzej", 3),
            ("cztery", 4),
            ("czterech", 4),
            ("pięć", 5),
            ("pięciu", 5),
            ("sześć", 6),
            ("siedem", 7),
            ("osiem", 8),
            ("dziewięć", 9),
            ("dziesięć", 10),
        ],
        "tr" => &[
            ("bir", 1),
            ("iki", 2),
            ("üç", 3),
            ("dört", 4),
            ("beş", 5),
            ("altı", 6),
            ("yedi", 7),
            ("sekiz", 8),
            ("dokuz", 9),
            ("on", 10),
        ],
        "ar" => &[
            ("واحد", 1),
            ("واحدة", 1),
            ("اثنان", 2),
            ("اثنين", 2),
            ("اثنتان", 2),
            ("ثلاثة", 3),
            ("ثلاث", 3),
            ("أربعة", 4),
            ("أربع", 4),
            ("خمسة", 5),
            ("خمس", 5),
            ("ستة", 6),
            ("سبعة", 7),
            ("ثمانية", 8),
            ("تسعة", 9),
            ("عشرة", 10),
        ],
        _ => &[],
    }
}

/// Languages written without spaces: numbers are read character by character
/// (十 is left out: 十分 means "enough", "very").
fn cjk(lang: &str) -> bool {
    matches!(lang, "zh" | "ja")
}

fn cjk_number(c: char) -> Option<u8> {
    Some(match c {
        '二' | '两' | '兩' => 2,
        '三' => 3,
        '四' => 4,
        '五' => 5,
        '六' => 6,
        '七' => 7,
        '八' => 8,
        '九' => 9,
        _ => return None,
    })
}

const ALL: [&str; 10] = ["en", "fr", "de", "es", "pt", "ru", "uk", "pl", "tr", "ar"];

fn words(text: &str) -> Vec<String> {
    text.split(|c: char| !(c.is_alphanumeric() || c == '\'' || c == 'ʼ'))
        .filter(|w| !w.is_empty())
        .map(|w| w.to_lowercase())
        .collect()
}

/// Standalone ASCII digit runs ("2", "2个", not "m4" or "1v1"): their values.
fn digits(text: &str) -> Vec<u32> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if !chars[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        let start = i;
        while i < chars.len() && chars[i].is_ascii_digit() {
            i += 1;
        }
        let ascii_word = |c: Option<&char>| c.is_some_and(|c| c.is_ascii_alphanumeric());
        let (before, after) = (
            start.checked_sub(1).and_then(|k| chars.get(k)),
            chars.get(i),
        );
        // Arabic-Indic digits are converted below; a letter glued to it is a name (m4, b2).
        if !ascii_word(before) && !ascii_word(after) {
            if let Ok(v) = chars[start..i].iter().collect::<String>().parse() {
                out.push(v);
            }
        }
    }
    out
}

/// Player counts said in `text` (2 to 10; `lang` "auto": every language).
fn counts(text: &str, lang: &str) -> Vec<u8> {
    let text: String = text
        .chars()
        .map(|c| match c {
            // Arabic-Indic and Persian digits.
            '٠'..='٩' => char::from(b'0' + (c as u32 - '٠' as u32) as u8),
            '۰'..='۹' => char::from(b'0' + (c as u32 - '۰' as u32) as u8),
            '’' => '\'',
            _ => c,
        })
        .collect();
    let mut out: Vec<u8> = digits(&text)
        .into_iter()
        .filter(|v| (2..=10).contains(v))
        .map(|v| v as u8)
        .collect();
    let langs: Vec<&str> = if lang == "auto" {
        ALL.to_vec()
    } else {
        vec![lang]
    };
    for w in words(&text) {
        // Arabic glues "and" (و) to the next word.
        let bare = w.strip_prefix('و').filter(|b| b.chars().count() > 2);
        for l in &langs {
            if let Some((_, v)) = number_words(l)
                .iter()
                .find(|(k, _)| *k == w || Some(*k) == bare)
            {
                out.push(*v);
            }
        }
    }
    if lang == "auto" || cjk(lang) {
        out.extend(text.chars().filter_map(cjk_number));
    }
    out.retain(|v| *v >= 2);
    out.sort_unstable();
    out.dedup();
    out
}

/// Sites said in `text`: (sure, maybe). A capital "A" opening a sentence
/// before a lower-case word may be the English article ("A guy on B"), and a
/// Cyrillic "А" opening a sentence the conjunction: only "maybe".
fn sites(text: &str) -> (Vec<char>, Vec<char>) {
    let chars: Vec<char> = text.chars().collect();
    let (mut sure, mut maybe) = (Vec::new(), Vec::new());
    for (i, &c) in chars.iter().enumerate() {
        let site = match c {
            'A' | 'А' => 'A',
            'B' | 'Б' | 'б' => 'B',
            _ => continue,
        };
        let before = i.checked_sub(1).map(|k| chars[k]);
        let after = chars.get(i + 1).copied();
        // Standalone: not a letter or digit of a word on either side (latin
        // or cyrillic; a CJK neighbour is fine: 去B点).
        let word = |c: Option<char>| {
            c.is_some_and(|c| {
                c.is_ascii_alphanumeric() || ('\u{0400}'..='\u{04FF}').contains(&c) || c == '\''
            })
        };
        if word(before) || word(after) {
            continue;
        }
        maybe.push(site);
        let opening = chars[..i]
            .iter()
            .rev()
            .find(|c| !c.is_whitespace())
            .map_or(true, |c| {
                matches!(c, '.' | '!' | '?' | '…' | '"' | '«' | '-' | '—')
            });
        let next_word = chars[i + 1..].iter().find(|c| !c.is_whitespace()).copied();
        let ambiguous = match c {
            'A' => opening && next_word.is_some_and(char::is_lowercase),
            'А' => opening,
            _ => false,
        };
        if !ambiguous {
            sure.push(site);
        }
    }
    for v in [&mut sure, &mut maybe] {
        v.sort_unstable();
        v.dedup();
    }
    (sure, maybe)
}

const LEFT: &[&str] = &[
    "left",
    "gauche",
    "links",
    "izquierda",
    "esquerda",
    "лево",
    "слева",
    "налево",
    "левый",
    "левая",
    "левее",
    "ліво",
    "зліва",
    "ліворуч",
    "лівий",
    "lewo",
    "lewa",
    "lewej",
    "sol",
    "solda",
    "sola",
    "يسار",
    "اليسار",
    "يساراً",
    "يسارا",
];
const RIGHT: &[&str] = &[
    "right",
    "droite",
    "rechts",
    "derecha",
    "direita",
    "право",
    "справа",
    "направо",
    "правый",
    "правая",
    "правее",
    "праворуч",
    "правий",
    "prawo",
    "prawa",
    "prawej",
    "sağ",
    "sağda",
    "sağa",
    "يمين",
    "اليمين",
    "يميناً",
    "يمينا",
];

/// (left, right) said in `text`.
fn sides(text: &str) -> (bool, bool) {
    let w = words(text);
    let has = |list: &[&str], han: char| {
        w.iter().any(|x| list.contains(&x.as_str())) || text.contains(han)
    };
    (has(LEFT, '左'), has(RIGHT, '右'))
}

/// Whether `translated` (in `target`) keeps the counts, sites and side of
/// `source` (in `source_lang`, or "auto").
pub fn check(source: &str, source_lang: &str, translated: &str, target: &str) -> Result<(), Issue> {
    // Counts: none added; none dropped, unless the source language is unsure
    // or the target is Arabic (dual nouns say "two" without a number word).
    let (src, out) = (counts(source, source_lang), counts(translated, target));
    let dropped_known = source_lang != "auto" && target != "ar";
    let cjk_shows = |v: &u8| !cjk(target) || *v != 10;
    if out.iter().any(|v| !src.contains(v))
        || (dropped_known
            && src
                .iter()
                .filter(|v| cjk_shows(v))
                .any(|v| !out.contains(v)))
    {
        return Err(Issue::Number);
    }
    let ((src_sure, src_maybe), (out_sure, out_maybe)) = (sites(source), sites(translated));
    if src_sure.iter().any(|s| !out_maybe.contains(s))
        || out_sure.iter().any(|s| !src_maybe.contains(s))
    {
        return Err(Issue::Site);
    }
    // Only a swap: "left" also means "remaining", "right" also "correct".
    let ((sl, sr), (ol, or)) = (sides(source), sides(translated));
    if (sl && !sr && or && !ol) || (sr && !sl && ol && !or) {
        return Err(Issue::Direction);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Faithful translations of real CS2 calls: all accepted.
    const FAITHFUL: &[(&str, &str, &str, &str)] = &[
        (
            "Двое на B, один лоу",
            "ru",
            "Deux sur B, un à peu de PV",
            "fr",
        ),
        ("Двое идут на Б", "ru", "Two are coming to B", "en"),
        (
            "two on B, one low",
            "en",
            "deux sur B, un à peu de PV",
            "fr",
        ),
        ("2 A long, rotate", "en", "2 au long A, rotation", "fr"),
        ("A guy is pushing B", "en", "Un gars pousse B", "fr"),
        (
            "A short, three of them",
            "en",
            "A short, ils sont trois",
            "fr",
        ),
        ("both of them are on A", "en", "les deux sont sur A", "fr"),
        (
            "drei auf B, einer links",
            "de",
            "three on B, one on the left",
            "en",
        ),
        (
            "dos por la izquierda, B",
            "es",
            "deux par la gauche, B",
            "fr",
        ),
        (
            "cinco no meio, todos para A",
            "pt",
            "five mid, everyone to A",
            "en",
        ),
        ("bomb down, he's 90", "en", "bombe au sol, il a 90", "fr"),
        (
            "I've got the AK, he has an M4",
            "en",
            "J'ai l'AK, il a une M4",
            "fr",
        ),
        ("Двое на B", "ru", "两个在B点", "zh"),
        ("three on A", "en", "Aに三人", "ja"),
        ("two on B", "en", "اثنان في B", "ar"),
        ("two players on B", "en", "لاعبان في B", "ar"),
        ("He's going right", "en", "Il va à droite", "fr"),
        (
            "one left, all right?",
            "en",
            "il en reste un, d'accord ?",
            "fr",
        ),
        ("go go go", "en", "on y va", "fr"),
        ("Двое на B, двое на B", "ru", "Deux sur B", "fr"),
        ("trois sur B", "auto", "three on B", "en"),
    ];

    #[test]
    fn faithful_translations_pass() {
        for (src, sl, out, tl) in FAITHFUL {
            assert_eq!(check(src, sl, out, tl), Ok(()), "{src} → {out}");
        }
    }

    #[test]
    fn a_changed_count_is_caught() {
        for (src, sl, out, tl) in [
            ("Двое на B", "ru", "Trois sur B", "fr"),
            ("two on B", "en", "sur B", "fr"),
            ("2 on B", "en", "3 sur B", "fr"),
            ("on B", "en", "deux sur B", "fr"),
            ("three on A", "en", "Aに二人", "ja"),
            ("two on B", "en", "ثلاثة في B", "ar"),
            ("drei auf B", "de", "two on B", "en"),
        ] {
            assert_eq!(check(src, sl, out, tl), Err(Issue::Number), "{src} → {out}");
        }
    }

    #[test]
    fn a_changed_site_is_caught() {
        for (src, sl, out, tl) in [
            ("two on B", "en", "deux sur A", "fr"),
            ("Двое на Б", "ru", "Deux sur A", "fr"),
            ("rush A", "en", "rush", "fr"),
            ("he is planting", "en", "il pose sur B", "fr"),
            ("Двое на B", "ru", "两个在A点", "zh"),
        ] {
            assert_eq!(check(src, sl, out, tl), Err(Issue::Site), "{src} → {out}");
        }
    }

    #[test]
    fn a_swapped_side_is_caught() {
        assert_eq!(
            check("He's going left", "en", "Il va à droite", "fr"),
            Err(Issue::Direction)
        );
        assert_eq!(
            check("справа двое", "ru", "two on the left", "en"),
            Err(Issue::Direction)
        );
        assert_eq!(check("left", "en", "左边", "zh"), Ok(()));
    }
}
