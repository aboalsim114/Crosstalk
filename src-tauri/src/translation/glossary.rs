//! CS glossary (F-33): terms kept as spoken ("smoke", "rush B") or given an
//! imposed rendering. Each term is replaced by a placeholder before translation
//! and restored after; if the model drops or repeats a placeholder, the line is
//! translated again without protection (never a mangled or invented output).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Term {
    pub source: String,
    /// Imposed French rendering; `None` keeps the term as spoken.
    pub target: Option<String>,
}

/// Placeholders look like codes that translation models copy verbatim.
fn placeholder(i: usize) -> String {
    format!("KZ{}", i + 1)
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Byte length of `term` matched case-insensitively at the start of `text`.
fn match_at(text: &str, term: &str) -> Option<usize> {
    let mut t = text.char_indices();
    let mut end = 0;
    for tc in term.chars() {
        let (i, c) = t.next()?;
        if !c.to_lowercase().eq(tc.to_lowercase()) {
            return None;
        }
        end = i + c.len_utf8();
    }
    Some(end)
}

/// Occurrences of glossary terms: (start, end, term index), longest term first,
/// whole words only, case-insensitive.
pub fn find(text: &str, terms: &[Term]) -> Vec<(usize, usize, usize)> {
    let mut order: Vec<usize> = (0..terms.len())
        .filter(|&i| !terms[i].source.trim().is_empty())
        .collect();
    order.sort_by_key(|&i| std::cmp::Reverse(terms[i].source.chars().count()));
    let mut found = Vec::new();
    let mut pos = 0;
    while pos < text.len() {
        let before = text[..pos].chars().next_back();
        let hit = (before.map_or(true, |c| !is_word(c)))
            .then(|| {
                order.iter().find_map(|&i| {
                    let len = match_at(&text[pos..], terms[i].source.trim())?;
                    let after = text[pos + len..].chars().next();
                    after
                        .map_or(true, |c| !is_word(c))
                        .then_some((pos, pos + len, i))
                })
            })
            .flatten();
        match hit {
            Some(h) => {
                found.push(h);
                pos = h.1;
            }
            None => pos += text[pos..].chars().next().map_or(1, char::len_utf8),
        }
    }
    found
}

/// Translates `text` with `engine`, protecting glossary terms. Returns the
/// French text and whether the terms could be protected.
pub fn translate(
    text: &str,
    terms: &[Term],
    mut engine: impl FnMut(&str) -> Result<String, String>,
) -> Result<(String, bool), String> {
    let hits = find(text, terms);
    if hits.is_empty() {
        return Ok((engine(text)?, true));
    }
    let mut protected = String::with_capacity(text.len());
    let mut last = 0;
    for (n, &(start, end, _)) in hits.iter().enumerate() {
        protected.push_str(&text[last..start]);
        protected.push_str(&placeholder(n));
        last = end;
    }
    protected.push_str(&text[last..]);
    let out = engine(&protected)?;
    match restore(&out, text, &hits, terms) {
        Some(restored) => Ok((restored, true)),
        None => Ok((engine(text)?, false)),
    }
}

fn restore(
    out: &str,
    text: &str,
    hits: &[(usize, usize, usize)],
    terms: &[Term],
) -> Option<String> {
    let mut result = out.to_string();
    // Highest numbers first: "KZ1" must not match inside "KZ12".
    for (n, &(start, end, i)) in hits.iter().enumerate().rev() {
        let ph = placeholder(n);
        let at: Vec<usize> = result
            .match_indices(&ph)
            .map(|(p, _)| p)
            .filter(|&p| {
                result[p + ph.len()..]
                    .chars()
                    .next()
                    .map_or(true, |c| !c.is_ascii_digit())
            })
            .collect();
        if at.len() != 1 {
            return None;
        }
        let rendering = terms[i].target.as_deref().unwrap_or(&text[start..end]);
        result.replace_range(at[0]..at[0] + ph.len(), rendering);
    }
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn term(s: &str, t: Option<&str>) -> Term {
        Term {
            source: s.into(),
            target: t.map(str::to_string),
        }
    }

    #[test]
    fn finds_whole_words_case_insensitively_longest_first() {
        let terms = [
            term("rush", None),
            term("rush B", None),
            term("smoke", None),
        ];
        let text = "Rush B now, SMOKE mid, smoker no, русскийsmoke";
        let hits = find(text, &terms);
        let words: Vec<&str> = hits.iter().map(|&(s, e, _)| &text[s..e]).collect();
        assert_eq!(words, ["Rush B", "SMOKE"]);
    }

    #[test]
    fn terms_survive_translation_with_their_rendering() {
        let terms = [term("smoke", None), term("rush B", Some("rush B !"))];
        let engine = |t: &str| Ok(t.replace("Two on", "Deux sur").replace("throw", "lance"));
        let (fr, ok) = translate("Two on rush B, throw Smoke", &terms, engine).unwrap();
        assert!(ok);
        assert_eq!(fr, "Deux sur rush B !, lance Smoke");
    }

    #[test]
    fn a_lost_placeholder_falls_back_to_an_unprotected_translation() {
        let terms = [term("smoke", None)];
        let mut calls = Vec::new();
        let (fr, ok) = translate("throw smoke", &terms, |t| {
            calls.push(t.to_string());
            Ok(t.replace("KZ1", "").replace("throw", "lance"))
        })
        .unwrap();
        assert!(!ok);
        assert_eq!(calls, ["throw KZ1", "throw smoke"]);
        assert_eq!(fr, "lance smoke");
    }

    #[test]
    fn duplicated_or_merged_placeholders_are_rejected() {
        let terms = [term("a1", None), term("b2", None)];
        let hits = find("a1 b2", &terms);
        assert!(restore("KZ1 KZ1 KZ2", "a1 b2", &hits, &terms).is_none());
        assert!(restore("KZ12", "a1 b2", &hits, &terms).is_none());
        assert_eq!(
            restore("KZ2 et KZ1", "a1 b2", &hits, &terms).as_deref(),
            Some("b2 et a1")
        );
    }

    #[test]
    fn no_term_means_one_plain_translation() {
        let (fr, ok) =
            translate("hello", &[term("smoke", None)], |t| Ok(format!("[{t}]"))).unwrap();
        assert_eq!((fr.as_str(), ok), ("[hello]", true));
    }
}
