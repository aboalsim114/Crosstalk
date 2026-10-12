use super::GlossaryEntry;
fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
fn word(c: char) -> bool {
    // Unicode \w also includes combining marks, connector punctuation and
    // join controls, which `char::is_alphanumeric` alone misses.
    static WORD: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    WORD.get_or_init(|| regex::Regex::new(r"^\w$").expect("constant Unicode class"))
        .is_match(&c.to_string())
}
pub struct Protected {
    pub xml: String,
    terms: Vec<(String, String)>,
}
impl Protected {
    pub fn new(text: &str, glossary: &[GlossaryEntry]) -> Self {
        let mut entries: Vec<_> = glossary.iter().collect();
        entries.sort_by_key(|e| std::cmp::Reverse(e.source.len()));
        let mut xml = String::from("<ctroot>");
        let mut terms = Vec::new();
        let mut pos = 0;
        while pos < text.len() {
            let matched = entries.iter().find(|e| {
                if !text[pos..].starts_with(&e.source) {
                    return false;
                }
                let end = pos + e.source.len();
                let before = text[..pos].chars().next_back();
                let after = text[end..].chars().next();
                !(e.source.chars().next().is_some_and(word) && before.is_some_and(word)
                    || e.source.chars().next_back().is_some_and(word) && after.is_some_and(word))
            });
            if let Some(e) = matched {
                let id = terms.len();
                xml.push_str(&format!("<ctg id=\"{id}\">{}</ctg>", escape(&e.source)));
                terms.push((
                    e.source.clone(),
                    e.target.clone().unwrap_or_else(|| e.source.clone()),
                ));
                pos += e.source.len();
            } else {
                let c = text[pos..].chars().next().expect("valid boundary");
                xml.push_str(&escape(&c.to_string()));
                pos += c.len_utf8();
            }
        }
        xml.push_str("</ctroot>");
        Self { xml, terms }
    }
    pub fn restore(&self, xml: &str) -> Result<String, ()> {
        let doc = roxmltree::Document::parse(xml).map_err(|_| ())?;
        let root = doc.root_element();
        if root.tag_name().name() != "ctroot"
            || root.tag_name().namespace().is_some()
            || root.namespaces().len() != 0
            || root.attributes().len() != 0
        {
            return Err(());
        }
        if doc.root().children().any(|n| n.is_comment() || n.is_pi()) {
            return Err(());
        }
        let mut seen = vec![false; self.terms.len()];
        let mut out = String::new();
        for node in root.children() {
            if node.is_text() {
                out.push_str(node.text().unwrap_or_default());
                continue;
            }
            if !node.is_element()
                || node.tag_name().name() != "ctg"
                || node.tag_name().namespace().is_some()
                || node.namespaces().len() != 0
                || node.attributes().len() != 1
            {
                return Err(());
            }
            let id: usize = node.attribute("id").ok_or(())?.parse().map_err(|_| ())?;
            let (source, target) = self.terms.get(id).ok_or(())?;
            let payload: String = node.children().filter_map(|child| child.text()).collect();
            if seen[id]
                || node.attribute("id") != Some(id.to_string().as_str())
                || node.children().any(|n| !n.is_text())
                || &payload != source
            {
                return Err(());
            }
            seen[id] = true;
            out.push_str(target);
        }
        if seen.iter().any(|v| !v) {
            return Err(());
        }
        if out.trim().is_empty() {
            return Err(());
        }
        Ok(out)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn entry(source: &str, target: Option<&str>) -> GlossaryEntry {
        GlossaryEntry {
            source: source.into(),
            target: target.map(str::to_owned),
        }
    }
    #[test]
    fn escapes_literals_and_preserves_terms() {
        let p = Protected::new(
            "<rush B> & smoke",
            &[entry("rush B", Some("attaque B")), entry("smoke", None)],
        );
        assert_eq!(p.restore(&p.xml).unwrap(), "<attaque B> & smoke");
    }
    #[test]
    fn unicode_boundary_and_longest_match() {
        let p = Protected::new(
            "smoker smoke rush B русскийsmoke",
            &[
                entry("rush", None),
                entry("rush B", None),
                entry("smoke", None),
            ],
        );
        assert_eq!(p.terms.len(), 2);
        assert_eq!(p.terms[1].0, "rush B");
    }
    #[test]
    fn rejects_altered_missing_duplicate_unknown_and_unexpected_markup() {
        let p = Protected::new("smoke", &[entry("smoke", None)]);
        for xml in [
            "<ctroot/>",
            "<ctroot><ctg id=\"1\">smoke</ctg></ctroot>",
            "<ctroot><ctg id=\"0\">smoke</ctg><ctg id=\"0\">smoke</ctg></ctroot>",
            "<ctroot><ctg id=\"0\">wrong</ctg></ctroot>",
            "<ctroot><other>smoke</other></ctroot>",
            "<!DOCTYPE ctroot [<!ENTITY x 'smoke'>]><ctroot>&x;</ctroot>",
        ] {
            assert!(p.restore(xml).is_err());
        }
    }
    #[test]
    fn rejects_empty_translation_without_terms() {
        let protected = Protected::new("Hello", &[]);
        assert!(protected.restore("<ctroot/>").is_err());
        assert!(protected.restore("<ctroot> \t\n </ctroot>").is_err());
    }
    #[test]
    fn combining_marks_are_word_continuations() {
        let protected = Protected::new("smoke\u{301} smoke", &[entry("smoke", None)]);
        assert_eq!(protected.terms.len(), 1);
    }
}
