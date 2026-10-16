//! Lexicon of Counter-Strike callouts (cahier des charges F-36).
//!
//! A message made only of known callouts ("CT, go, go", "rush", "eco") is
//! translated from this table into French, instantly and exactly, without
//! asking the LLM (no latency, no API cost); anything else goes to the LLM.
//! Nothing absent from the original is added.

/// Exact segments (lowercase, without punctuation) → French.
const PHRASES: &[(&str, &str)] = &[
    // --- English ---
    ("go", "on y va"),
    ("go go", "on y va"),
    ("go go go", "on y va"),
    ("lets go", "on y va"),
    ("let's go", "on y va"),
    ("gogo", "on y va"),
    ("rush", "rush"),
    ("push", "push"),
    ("rotate", "rotation"),
    ("rotating", "je tourne"),
    ("fall back", "repliez-vous"),
    ("back", "repliez-vous"),
    ("hold", "tenez la position"),
    ("wait", "attendez"),
    ("wait for me", "attendez-moi"),
    ("stop", "stop"),
    ("stay", "restez"),
    ("save", "save"),
    ("save it", "save"),
    ("eco", "éco"),
    ("force", "force buy"),
    ("force buy", "force buy"),
    ("full buy", "full buy"),
    ("buy", "achetez"),
    ("drop", "drop"),
    ("drop me", "drop-moi une arme"),
    ("drop pls", "drop-moi une arme, stp"),
    ("one low", "un ennemi à peu de PV"),
    ("1 low", "un ennemi à peu de PV"),
    ("two low", "deux ennemis à peu de PV"),
    ("2 low", "deux ennemis à peu de PV"),
    ("low", "peu de PV"),
    ("he's low", "il a peu de PV"),
    ("hes low", "il a peu de PV"),
    ("lit", "touché (gros dégâts)"),
    ("one lit", "un touché (gros dégâts)"),
    ("he's lit", "il est touché (gros dégâts)"),
    ("hes lit", "il est touché (gros dégâts)"),
    ("tagged", "touché"),
    ("one shot", "un coup et il tombe"),
    ("last one", "le dernier"),
    ("last", "le dernier"),
    ("one left", "il en reste un"),
    ("two left", "il en reste deux"),
    ("1 left", "il en reste un"),
    ("2 left", "il en reste deux"),
    ("bomb", "bombe"),
    ("bomb a", "bombe sur A"),
    ("bomb b", "bombe sur B"),
    ("bomb down", "bombe au sol"),
    ("planted", "bombe posée"),
    ("planting", "il pose la bombe"),
    ("he's planting", "il pose la bombe"),
    ("defusing", "il désamorce"),
    ("he's defusing", "il désamorce"),
    ("hes defusing", "il désamorce"),
    ("defuse", "désamorce"),
    ("behind", "derrière"),
    ("behind you", "derrière toi"),
    ("on me", "sur moi"),
    ("help", "aide"),
    ("help me", "aidez-moi"),
    ("careful", "attention"),
    ("watch out", "attention"),
    ("flash", "flash"),
    ("flash out", "flash lancée"),
    ("smoke out", "smoke lancée"),
    ("molly out", "molotov lancé"),
    ("nade out", "grenade lancée"),
    ("he out", "HE lancée"),
    ("flashing", "je flash"),
    ("smoking", "je smoke"),
    ("dead", "mort"),
    ("he's dead", "il est mort"),
    ("killed", "tué"),
    ("kill", "tué"),
    ("smoke", "smoke"),
    ("molly", "molotov"),
    ("nade", "grenade"),
    ("nice", "bien joué"),
    ("nice one", "bien joué"),
    ("nice shot", "joli tir"),
    ("wp", "bien joué"),
    ("well played", "bien joué"),
    ("nt", "bien tenté"),
    ("nice try", "bien tenté"),
    ("gg", "gg"),
    ("gg wp", "gg, bien joué"),
    ("ggwp", "gg, bien joué"),
    ("ez", "ez"),
    ("ty", "merci"),
    ("thx", "merci"),
    ("thanks", "merci"),
    ("sorry", "désolé"),
    ("sry", "désolé"),
    ("np", "pas de souci"),
    ("ok", "ok"),
    ("yes", "oui"),
    ("no", "non"),
    ("ct", "CT"),
    ("t", "T"),
    ("cts", "les CT"),
    ("ts", "les T"),
    ("awp", "AWP"),
    ("mid", "mid"),
    ("a", "A"),
    ("b", "B"),
    // --- Russian ---
    ("го", "on y va"),
    ("гоу", "on y va"),
    ("го го", "on y va"),
    ("пошли", "on y va"),
    ("раш", "rush"),
    ("раш а", "rush A"),
    ("раш б", "rush B"),
    ("ротейт", "rotation"),
    ("назад", "repliez-vous"),
    ("стоим", "on tient la position"),
    ("жди", "attends"),
    ("ждём", "on attend"),
    ("ждем", "on attend"),
    ("стоп", "stop"),
    ("сейв", "save"),
    ("эко", "éco"),
    ("форс", "force buy"),
    ("фулл бай", "full buy"),
    ("дроп", "drop"),
    ("дропни", "drop-moi une arme"),
    ("лоу", "peu de PV"),
    ("один лоу", "un ennemi à peu de PV"),
    ("минус", "un de moins"),
    ("минус один", "un de moins"),
    ("минус два", "deux de moins"),
    ("последний", "le dernier"),
    ("бомба", "bombe"),
    ("бомба а", "bombe sur A"),
    ("бомба б", "bombe sur B"),
    ("плент", "bombe posée"),
    ("дефуз", "il désamorce"),
    ("дефузит", "il désamorce"),
    ("сзади", "derrière"),
    ("помогите", "aidez-moi"),
    ("аккуратно", "attention"),
    ("осторожно", "attention"),
    ("флеш", "flash"),
    ("флешка", "flash"),
    ("смок", "smoke"),
    ("молик", "molotov"),
    ("красава", "bien joué"),
    ("молодец", "bien joué"),
    ("гг", "gg"),
    ("изи", "ez"),
    ("спасибо", "merci"),
    ("спс", "merci"),
    ("сорян", "désolé"),
    ("да", "oui"),
    ("нет", "non"),
    ("кт", "CT"),
    ("авп", "AWP"),
    ("мид", "mid"),
    ("а", "A"),
    ("б", "B"),
];

fn count(word: &str) -> Option<&'static str> {
    Some(match word {
        "one" | "1" | "один" | "одного" => "un",
        "two" | "2" | "двое" | "два" | "двоих" => "deux",
        "three" | "3" | "трое" | "три" | "троих" => "trois",
        "four" | "4" | "четверо" | "четыре" => "quatre",
        "five" | "5" | "пятеро" | "пять" => "cinq",
        "all" | "все" => "tous",
        _ => return None,
    })
}

fn place(word: &str) -> Option<&'static str> {
    Some(match word {
        "a" | "а" => "sur A",
        "b" | "б" => "sur B",
        "mid" | "мид" => "au mid",
        "long" | "лонг" => "au long",
        "short" | "шорт" => "au short",
        "ramp" | "рампа" => "à la rampe",
        "apps" | "aps" => "aux apparts",
        "banana" | "банан" => "à la banane",
        "connector" | "кон" => "au connecteur",
        "ct" | "кт" => "côté CT",
        _ => return None,
    })
}

/// "two B", "2 on B", "двое на Б", "awp mid", "bomb B"…
fn pattern(seg: &str) -> Option<String> {
    let words: Vec<&str> = seg
        .split_whitespace()
        .filter(|w| !matches!(*w, "on" | "at" | "in" | "на" | "в"))
        .collect();
    match words.as_slice() {
        [n, p] if count(n).is_some() && place(p).is_some() => {
            Some(format!("{} {}", count(n)?, place(p)?))
        }
        [w, p] if matches!(*w, "awp" | "авп") && place(p).is_some() => {
            Some(format!("AWP {}", place(p)?))
        }
        [w, p] if matches!(*w, "rush" | "раш" | "push") => {
            let site = match *p {
                "a" | "а" => "A",
                "b" | "б" => "B",
                "mid" | "мид" => "mid",
                "long" | "лонг" => "long",
                "short" | "шорт" => "short",
                _ => return None,
            };
            Some(format!(
                "{} {site}",
                if *w == "push" { "push" } else { "rush" }
            ))
        }
        [w, p] if matches!(*w, "rotate" | "ротейт") && place(p).is_some() => {
            Some(format!("rotation {}", place(p)?))
        }
        _ => None,
    }
}

fn segment(seg: &str) -> Option<String> {
    PHRASES
        .iter()
        .find(|(k, _)| *k == seg)
        .map(|(_, v)| v.to_string())
        .or_else(|| pattern(seg))
}

/// French for a message made only of known callouts; `None` otherwise (the
/// model translates it). Segments are separated by `, . ! ? ;`.
pub fn translate(lang: &str, text: &str) -> Option<String> {
    if lang != "en" && lang != "ru" {
        return None;
    }
    let lower = text.to_lowercase().replace('ё', "е");
    let segments: Vec<String> = lower
        .split([',', '.', '!', '?', ';', '…'])
        .map(|s| s.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|s| !s.is_empty())
        .collect();
    if segments.is_empty() || segments.len() > 6 {
        return None;
    }
    // The recogniser may cut "smoke out" (grenade thrown) into "smoke, out".
    let mut merged: Vec<String> = Vec::with_capacity(segments.len());
    for s in segments {
        match merged.last_mut() {
            Some(prev)
                if s == "out"
                    && matches!(prev.as_str(), "smoke" | "flash" | "molly" | "nade" | "he") =>
            {
                prev.push_str(" out")
            }
            _ => merged.push(s),
        }
    }
    let segments = merged;
    let mut out: Vec<String> = Vec::new();
    for s in &segments {
        let fr = segment(s)?;
        // "go, go" reads "on y va", not "on y va, on y va".
        if out.last() != Some(&fr) {
            out.push(fr);
        }
    }
    Some(out.join(", "))
}

/// Removes a leading dialogue dash some models add ("- Deux sur B.").
pub fn clean_output(text: &str) -> String {
    let t = text.trim();
    let t = t.trim_start_matches(['-', '–', '—']).trim_start();
    t.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translates_callouts_exactly() {
        assert_eq!(
            translate("en", "CT, go, go.").as_deref(),
            Some("CT, on y va")
        );
        assert_eq!(
            translate("en", "CT, one low, lit.").as_deref(),
            Some("CT, un ennemi à peu de PV, touché (gros dégâts)")
        );
        assert_eq!(translate("en", "two B").as_deref(), Some("deux sur B"));
        assert_eq!(translate("en", "2 on mid!").as_deref(), Some("deux au mid"));
        assert_eq!(translate("en", "AWP long").as_deref(), Some("AWP au long"));
        assert_eq!(translate("en", "rush B").as_deref(), Some("rush B"));
        assert_eq!(translate("en", "gg wp").as_deref(), Some("gg, bien joué"));
        assert_eq!(translate("ru", "Двое на Б").as_deref(), Some("deux sur B"));
        assert_eq!(
            translate("ru", "Го, раш Б!").as_deref(),
            Some("on y va, rush B")
        );
        assert_eq!(
            translate("ru", "минус один, последний").as_deref(),
            Some("un de moins, le dernier")
        );
    }

    #[test]
    fn grenades_thrown() {
        assert_eq!(
            translate("en", "B, mid, smoke, out.").as_deref(),
            Some("B, mid, smoke lancée")
        );
        assert_eq!(
            translate("en", "flash out").as_deref(),
            Some("flash lancée")
        );
        assert_eq!(translate("en", "out"), None);
    }

    #[test]
    fn leaves_real_sentences_to_the_model() {
        assert_eq!(translate("en", "they are going B with an AWP"), None);
        assert_eq!(translate("en", "rotate, shoot,"), None);
        assert_eq!(translate("ru", "Двое идут на B, у них AWP"), None);
        assert_eq!(translate("fr", "go"), None);
        assert_eq!(translate("en", ""), None);
    }

    #[test]
    fn cleans_subtitle_dashes() {
        assert_eq!(
            clean_output("- T'es pas un bon ami."),
            "T'es pas un bon ami."
        );
        assert_eq!(clean_output("— Deux sur B"), "Deux sur B");
        assert_eq!(clean_output("Deux-trois"), "Deux-trois");
    }
}
