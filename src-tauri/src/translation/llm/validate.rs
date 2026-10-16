//! The model's answer must be a translation of the line: no explanation,
//! refusal, empty or runaway text. Nothing invented is ever shown.

/// Rejects answers that are not a translation of the line (explanations,
/// refusals, empty or runaway text), cleaned of quotes and prefixes.
pub fn plausible(input: &str, output: &str) -> Result<String, String> {
    let mut out = output.trim().to_string();
    for prefix in ["translation:", "traduction :", "traduction:", "translated:"] {
        // `get`: the answer may start with a multi-byte letter ("Ils ont posé…").
        if out
            .get(..prefix.len())
            .is_some_and(|p| p.eq_ignore_ascii_case(prefix))
        {
            out = out[prefix.len()..].trim().to_string();
        }
    }
    for (open, close) in [("\"", "\""), ("«", "»"), ("“", "”"), ("'", "'")] {
        let quoted_in = input.trim().starts_with(open);
        if !quoted_in && out.starts_with(open) && out.ends_with(close) && out.len() > 2 {
            out = out[open.len()..out.len() - close.len()].trim().to_string();
        }
    }
    if out.is_empty() {
        return Err("réponse incohérente (vide)".into());
    }
    let n_in = input.chars().count();
    let n_out = out.chars().count();
    if n_out > n_in * 4 + 40 {
        return Err("réponse incohérente (trop longue)".into());
    }
    if n_in >= 16 && n_out * 5 < n_in {
        return Err("réponse incohérente (trop courte)".into());
    }
    let lines_in = input
        .lines()
        .filter(|l| !l.trim().is_empty())
        .count()
        .max(1);
    let lines_out = out.lines().filter(|l| !l.trim().is_empty()).count();
    if lines_out > lines_in + 1 {
        return Err("réponse incohérente (explications)".into());
    }
    let lower_in = input.to_lowercase();
    let lower_out = out.to_lowercase();
    const META: [&str; 8] = [
        "as an ai",
        "i'm sorry",
        "i cannot",
        "i can't",
        "en tant qu'ia",
        "je ne peux pas",
        "désolé, je",
        "here is the translation",
    ];
    if META
        .iter()
        .any(|m| lower_out.contains(m) && !lower_in.contains(m))
    {
        return Err("réponse incohérente (refus ou commentaire)".into());
    }
    Ok(out)
}
