//! Quick replies: the usual CS2 calls, written by hand in English and Russian.
//! A message that matches one of them (accents, case and punctuation aside) is
//! translated instantly and exactly; anything else goes to the Opus-MT model.

use std::sync::OnceLock;

use regex::Regex;
use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Phrase {
    pub id: &'static str,
    /// What the button shows (French).
    pub fr: &'static str,
    #[serde(skip)]
    pub alts: &'static [&'static str],
    #[serde(skip)]
    pub en: &'static str,
    #[serde(skip)]
    pub ru: &'static str,
    /// Shown as a button in the reply window.
    pub quick: bool,
}

const fn p(
    id: &'static str,
    fr: &'static str,
    alts: &'static [&'static str],
    en: &'static str,
    ru: &'static str,
    quick: bool,
) -> Phrase {
    Phrase {
        id,
        fr,
        alts,
        en,
        ru,
        quick,
    }
}

pub const PHRASES: &[Phrase] = &[
    p(
        "hello",
        "Salut à tous",
        &[
            "salut",
            "bonjour",
            "slt",
            "coucou",
            "salut tout le monde",
            "bonjour a tous",
        ],
        "Hi all!",
        "Всем привет!",
        false,
    ),
    p(
        "eco",
        "Éco",
        &["eco", "on eco", "on fait eco", "eco round"],
        "Eco round",
        "Эко раунд",
        true,
    ),
    p(
        "full",
        "On achète tout",
        &[
            "full buy",
            "full achat",
            "achetez tout",
            "on full buy",
            "on achete tout",
        ],
        "Full buy",
        "Фулл бай",
        true,
    ),
    p(
        "force",
        "Force buy",
        &["on force", "force", "on force buy"],
        "Force buy",
        "Форс бай",
        false,
    ),
    p(
        "save",
        "Sauvez vos armes",
        &["save", "on save", "sauvez", "sauvez les armes"],
        "Save your weapons",
        "Сейвим оружие",
        true,
    ),
    p(
        "dropme",
        "Drop-moi une arme stp",
        &[
            "drop moi",
            "drop moi stp",
            "drop stp",
            "tu peux me drop",
            "drop moi une arme",
            "drop moi une arme s il te plait",
        ],
        "Can you drop me a weapon please?",
        "Скинь мне оружие, пожалуйста",
        true,
    ),
    p(
        "idrop",
        "Je te drop",
        &["je drop", "je te drop une arme", "je drop une arme"],
        "I'll drop you a weapon",
        "Сейчас скину тебе оружие",
        false,
    ),
    p(
        "rushb",
        "Rush B",
        &["on rush b", "rush b go", "rush b on y va", "rush b"],
        "Rush B, go!",
        "Раш Б, го!",
        true,
    ),
    p(
        "rusha",
        "Rush A",
        &["on rush a", "rush a go", "rush a on y va", "rush a"],
        "Rush A, go!",
        "Раш А, го!",
        true,
    ),
    p(
        "wait",
        "Attendez",
        &["attends", "on attend", "wait", "attendez moi"],
        "Wait",
        "Ждите",
        true,
    ),
    p(
        "go",
        "On y va",
        &["go", "allez go", "on go", "allez"],
        "Let's go",
        "Го",
        false,
    ),
    p(
        "slow",
        "On joue lent",
        &["jouez lent", "on joue lentement"],
        "Let's play slow",
        "Играем медленно",
        false,
    ),
    p(
        "fast",
        "On joue vite",
        &["jouez vite", "on joue rapide"],
        "Let's play fast",
        "Играем быстро",
        false,
    ),
    p(
        "rotate",
        "Rotate",
        &["on rotate", "tournez", "on tourne", "rotatez"],
        "Rotate",
        "Ротейт",
        true,
    ),
    p(
        "low",
        "Il est à 1 PV",
        &[
            "un low",
            "il est low",
            "il est one",
            "one hp",
            "il lui reste rien",
            "il est a 1 pv",
            "il est a peu de vie",
            "il est one shot",
        ],
        "He's one HP",
        "Он лоу",
        true,
    ),
    p(
        "behind",
        "Derrière toi",
        &["derriere toi", "attention derriere toi", "derriere"],
        "Behind you!",
        "Сзади тебя!",
        false,
    ),
    p(
        "nice",
        "Bien joué",
        &["bien joue", "nice", "joli", "gg wp", "bien jouer"],
        "Nice one!",
        "Красиво!",
        true,
    ),
    p(
        "nt",
        "Bien tenté",
        &["bien tente", "nt", "pas de chance", "bien essaye"],
        "Nice try",
        "Хорошая попытка",
        true,
    ),
    p(
        "sorry",
        "Désolé",
        &["desole", "pardon", "dsl", "my bad", "c est ma faute"],
        "Sorry, my bad",
        "Извини, моя ошибка",
        true,
    ),
    p(
        "thanks",
        "Merci",
        &["merci beaucoup", "thx", "merci a toi"],
        "Thanks!",
        "Спасибо!",
        true,
    ),
    p(
        "french",
        "Je suis français, je ne parle pas russe",
        &[
            "je suis francais",
            "je parle pas russe",
            "je ne parle pas russe",
            "je suis francais je parle pas russe",
        ],
        "I'm French, I don't speak Russian",
        "Я француз, не говорю по-русски",
        true,
    ),
    p(
        "english",
        "On peut parler anglais ?",
        &[
            "parle anglais",
            "parlez anglais",
            "english please",
            "on parle anglais",
            "vous parlez anglais",
        ],
        "Can we speak English?",
        "Можем говорить по-английски?",
        true,
    ),
    p(
        "awp",
        "Je prends l'AWP",
        &[
            "je joue awp",
            "je prends l awp",
            "j ai l awp",
            "je prends awp",
        ],
        "I'll play AWP",
        "Я возьму AWP",
        false,
    ),
    p(
        "planting",
        "Il plante",
        &["ils plantent", "il pose la bombe", "il pose"],
        "He's planting!",
        "Он ставит бомбу!",
        false,
    ),
    p(
        "defuse",
        "Je désamorce",
        &[
            "je defuse",
            "defuse",
            "je desamorce",
            "je defuse couvrez moi",
        ],
        "I'm defusing, cover me",
        "Дефьюжу, прикройте",
        false,
    ),
    p(
        "bomba",
        "Bombe en A",
        &["bombe a", "bombe sur a", "c4 en a", "la bombe est en a"],
        "Bomb is A",
        "Бомба на А",
        false,
    ),
    p(
        "bombb",
        "Bombe en B",
        &["bombe b", "bombe sur b", "c4 en b", "la bombe est en b"],
        "Bomb is B",
        "Бомба на Б",
        false,
    ),
    p(
        "clear",
        "C'est clean",
        &["clean", "c est clean", "clear", "c est bon", "c est vide"],
        "Clear",
        "Чисто",
        false,
    ),
    p(
        "dead",
        "Je suis mort",
        &["mort", "je meurs", "je suis mort desole"],
        "I'm dead",
        "Я умер",
        false,
    ),
    p(
        "holda",
        "Je tiens A",
        &["je garde a", "je reste a", "je tiens le a"],
        "I'm holding A",
        "Держу А",
        false,
    ),
    p(
        "holdb",
        "Je tiens B",
        &["je garde b", "je reste b", "je tiens le b"],
        "I'm holding B",
        "Держу Б",
        false,
    ),
    p(
        "mid",
        "Ils sont mid",
        &["ils sont au mid", "ils passent mid"],
        "They're mid",
        "Они на миду",
        false,
    ),
    p(
        "smoke",
        "Je smoke",
        &["je lance une smoke", "smoke", "je mets une smoke"],
        "Smoking",
        "Кидаю смок",
        false,
    ),
    p(
        "flash",
        "Je flash",
        &["je lance une flash", "flash"],
        "Flashing",
        "Кидаю флешку",
        false,
    ),
    p(
        "molly",
        "Je molotov",
        &["molotov", "molly", "je molly", "je lance une molotov"],
        "Molly out",
        "Кидаю молотов",
        false,
    ),
    p(
        "help",
        "Aidez-moi",
        &["aide moi", "aidez moi", "help", "a l aide"],
        "Help me!",
        "Помогите!",
        false,
    ),
    p(
        "last",
        "Je suis le dernier",
        &["je suis seul", "clutch", "je suis le dernier en vie"],
        "I'm the last one alive",
        "Я последний",
        false,
    ),
    p(
        "quiet",
        "Silence svp",
        &["silence", "chut", "taisez vous", "silence s il vous plait"],
        "Quiet please",
        "Тише, пожалуйста",
        false,
    ),
    p("gg", "GG", &["gg", "good game"], "GG", "GG", true),
    p(
        "win",
        "On gagne ce round",
        &["on gagne celui la", "on prend ce round"],
        "Let's win this one!",
        "Выигрываем этот раунд!",
        false,
    ),
];

/// Lower case, no accents, apostrophes and punctuation as spaces, single spaces.
pub fn normalize(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars().flat_map(char::to_lowercase) {
        let c = match c {
            'à' | 'â' | 'ä' | 'á' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'î' | 'ï' | 'í' => 'i',
            'ô' | 'ö' | 'ó' => 'o',
            'ù' | 'û' | 'ü' | 'ú' => 'u',
            'ç' => 'c',
            'œ' => {
                out.push('o');
                'e'
            }
            c if c.is_alphanumeric() => c,
            _ => ' ',
        };
        out.push(c);
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// "deux en B", "ils sont 3 mid", "un sur A"…
fn count_call(norm: &str, target: &str) -> Option<String> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r"^(?:ils sont |il y en a |y en a |y a )?(un|une|1|deux|2|trois|3|quatre|4|cinq|5)(?: mecs?| ennemis?| gars| joueurs?)? (?:en |sur |a |au |vers |le )?(a|b|mid|le mid)$")
            .unwrap()
    });
    let c = re.captures(norm)?;
    let n = match &c[1] {
        "un" | "une" | "1" => 0,
        "deux" | "2" => 1,
        "trois" | "3" => 2,
        "quatre" | "4" => 3,
        _ => 4,
    };
    let site = c[2].trim_start_matches("le ");
    Some(match target {
        "ru" => {
            let who = ["Один", "Двое", "Трое", "Четверо", "Пятеро"][n];
            let place = match site {
                "a" => "на А",
                "b" => "на Б",
                _ => "на миду",
            };
            format!("{who} {place}")
        }
        _ => {
            let who = ["One", "Two", "Three", "Four", "Five"][n];
            let place = match site {
                "a" => "A",
                "b" => "B",
                _ => "mid",
            };
            format!("{who} {place}")
        }
    })
}

/// The hand-written translation of a usual call, if `text` is one.
pub fn lookup(text: &str, target: &str) -> Option<String> {
    let norm = normalize(text);
    if norm.is_empty() {
        return None;
    }
    let pick = |p: &Phrase| match target {
        "ru" => p.ru.to_string(),
        _ => p.en.to_string(),
    };
    PHRASES
        .iter()
        .find(|p| normalize(p.fr) == norm || p.alts.contains(&norm.as_str()))
        .map(pick)
        .or_else(|| count_call(&norm, target))
}

pub fn by_id(id: &str) -> Option<&'static Phrase> {
    PHRASES.iter().find(|p| p.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_like_people_type() {
        assert_eq!(
            normalize("  Drop-moi une arme, STP !"),
            "drop moi une arme stp"
        );
        assert_eq!(normalize("C'est clean"), "c est clean");
        assert_eq!(normalize("Désolé…"), "desole");
    }

    #[test]
    fn usual_calls_are_exact() {
        assert_eq!(lookup("éco", "en").as_deref(), Some("Eco round"));
        assert_eq!(lookup("Eco !", "ru").as_deref(), Some("Эко раунд"));
        assert_eq!(
            lookup("drop moi stp", "ru").as_deref(),
            Some("Скинь мне оружие, пожалуйста")
        );
        assert_eq!(lookup("Bien joué !", "en").as_deref(), Some("Nice one!"));
        assert_eq!(
            lookup("Je suis français, je ne parle pas russe", "ru").as_deref(),
            Some("Я француз, не говорю по-русски")
        );
        assert_eq!(lookup("on prend le mid et après on verra", "en"), None);
    }

    #[test]
    fn counts_on_a_site() {
        assert_eq!(lookup("deux en B", "en").as_deref(), Some("Two B"));
        assert_eq!(
            lookup("Ils sont 3 mid", "ru").as_deref(),
            Some("Трое на миду")
        );
        assert_eq!(lookup("un sur A", "ru").as_deref(), Some("Один на А"));
        assert_eq!(
            lookup("y a cinq mecs en b", "en").as_deref(),
            Some("Five B")
        );
        assert_eq!(lookup("deux en C", "en"), None);
    }

    #[test]
    fn every_phrase_is_complete_and_reachable() {
        let mut ids = std::collections::HashSet::new();
        for p in PHRASES {
            assert!(ids.insert(p.id), "duplicate {}", p.id);
            assert!(!p.en.is_empty() && !p.ru.is_empty(), "{}", p.id);
            assert_eq!(lookup(p.fr, "en").as_deref(), Some(p.en), "{}", p.id);
            for a in p.alts {
                assert_eq!(normalize(a), *a, "alt not normalized: {a}");
            }
        }
        assert!(PHRASES.iter().filter(|p| p.quick).count() >= 12);
    }
}
