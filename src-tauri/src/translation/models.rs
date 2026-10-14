//! Translation model catalog: Helsinki-NLP Opus-MT (Apache-2.0), one model
//! per language pair, stored in `<models dir>/opus-mt-<src>-<tgt>/` with the
//! files of the Hugging Face repository (`scripts/download-models.ps1`).
//!
//! The user chooses the language Crosstalk translates into (Langues › Ma
//! langue) and the languages of the players. A pair without a direct model
//! goes through English (two models in a row, e.g. German → English →
//! Russian). Polish and Turkish have models into the other languages but none
//! from English, so they are players' languages only.

use std::path::{Path, PathBuf};

use serde::Serialize;

#[derive(Debug, PartialEq, Eq)]
pub struct PairSpec {
    pub source: &'static str,
    pub target: &'static str,
    pub repo: &'static str,
    pub dir: &'static str,
    /// Exact sizes on Hugging Face: a different size is an interrupted download.
    pub files: [(&'static str, u64); 5],
}

/// Languages a player may speak or write (Whisper and the chat detection).
pub const LANGUAGES: [&str; 8] = ["fr", "en", "ru", "de", "es", "uk", "pl", "tr"];
/// Languages Crosstalk can translate into (the user's own language).
pub const TARGETS: [&str; 6] = ["fr", "en", "ru", "de", "es", "uk"];
pub const DEFAULT_TARGET: &str = "fr";
pub const DEFAULT_SOURCES: [&str; 2] = ["ru", "en"];
/// Language of the relay when no direct model exists.
pub const PIVOT: &str = "en";

macro_rules! pair {
    ($s:literal, $t:literal, [$c:expr, $v:expr, $ss:expr, $ts:expr, $m:expr]) => {
        PairSpec {
            source: $s,
            target: $t,
            repo: concat!("Helsinki-NLP/opus-mt-", $s, "-", $t),
            dir: concat!("opus-mt-", $s, "-", $t),
            files: [
                ("config.json", $c),
                ("vocab.json", $v),
                ("source.spm", $ss),
                ("target.spm", $ts),
                ("pytorch_model.bin", $m),
            ],
        }
    };
}

/// Every model Crosstalk can use, with the sizes published on Hugging Face
/// (October 2026). All share one vocabulary between source and target and
/// need no target-language token: the format the Marian engine reads.
pub const CATALOG: &[PairSpec] = &[
    pair!(
        "fr",
        "en",
        [1_416, 1_339_166, 802_397, 778_395, 300_827_685]
    ),
    pair!(
        "fr",
        "ru",
        [1_381, 2_658_919, 820_963, 1_063_848, 310_806_561]
    ),
    pair!(
        "fr",
        "de",
        [1_381, 1_464_136, 812_399, 827_737, 304_190_913]
    ),
    pair!(
        "fr",
        "es",
        [1_381, 2_337_705, 811_963, 818_689, 332_239_713]
    ),
    pair!(
        "fr",
        "uk",
        [1_381, 2_443_170, 832_408, 1_012_055, 305_721_705]
    ),
    pair!(
        "en",
        "fr",
        [1_416, 1_339_166, 778_395, 802_397, 300_827_685]
    ),
    pair!(
        "en",
        "ru",
        [1_381, 2_601_758, 802_781, 1_080_169, 306_991_893]
    ),
    pair!(
        "en",
        "de",
        [1_335, 1_273_232, 768_489, 796_845, 297_928_209]
    ),
    pair!(
        "en",
        "es",
        [1_473, 1_590_040, 801_636, 825_924, 312_087_523]
    ),
    pair!(
        "en",
        "uk",
        [1_381, 2_367_710, 808_645, 1_007_605, 305_081_481]
    ),
    pair!(
        "ru",
        "fr",
        [1_416, 2_658_919, 1_063_848, 820_963, 310_806_561]
    ),
    pair!(
        "ru",
        "en",
        [1_381, 2_601_758, 1_080_169, 802_781, 306_991_893]
    ),
    pair!(
        "ru",
        "es",
        [1_381, 2_619_455, 1_060_731, 829_382, 308_863_317]
    ),
    pair!(
        "ru",
        "uk",
        [1_394, 3_359_419, 1_049_819, 1_031_670, 297_263_361]
    ),
    pair!(
        "de",
        "fr",
        [1_381, 1_464_136, 827_737, 812_399, 304_190_913]
    ),
    pair!(
        "de",
        "en",
        [1_381, 1_273_232, 796_845, 768_489, 297_928_209]
    ),
    pair!(
        "de",
        "es",
        [1_381, 1_465_772, 828_663, 818_345, 304_494_609]
    ),
    pair!(
        "de",
        "uk",
        [1_394, 2_510_373, 846_351, 1_031_063, 307_002_153]
    ),
    pair!(
        "es",
        "fr",
        [1_381, 2_337_705, 818_689, 811_963, 332_239_713]
    ),
    pair!(
        "es",
        "en",
        [1_438, 1_590_040, 825_924, 801_636, 312_087_523]
    ),
    pair!(
        "es",
        "ru",
        [1_381, 2_619_455, 829_382, 1_060_731, 308_863_317]
    ),
    pair!(
        "es",
        "de",
        [1_381, 1_465_772, 818_345, 828_663, 304_494_609]
    ),
    pair!(
        "es",
        "uk",
        [1_394, 2_503_645, 840_419, 1_027_691, 306_872_877]
    ),
    pair!(
        "uk",
        "fr",
        [1_381, 2_443_170, 1_012_055, 832_408, 305_721_705]
    ),
    pair!(
        "uk",
        "en",
        [1_381, 2_367_710, 1_007_605, 808_645, 305_081_481]
    ),
    pair!(
        "uk",
        "ru",
        [1_394, 3_359_419, 1_031_670, 1_049_819, 297_263_361]
    ),
    pair!(
        "uk",
        "de",
        [1_394, 2_510_373, 1_031_063, 846_351, 307_002_153]
    ),
    pair!(
        "uk",
        "es",
        [1_381, 2_423_810, 1_008_282, 833_438, 305_801_733]
    ),
    pair!(
        "pl",
        "en",
        [1_381, 1_515_249, 834_921, 785_212, 308_863_317]
    ),
    pair!(
        "pl",
        "fr",
        [1_381, 1_462_731, 837_215, 808_576, 301_927_557]
    ),
    pair!(
        "pl",
        "de",
        [1_381, 1_467_369, 839_333, 823_900, 302_709_369]
    ),
    pair!(
        "pl",
        "es",
        [1_381, 1_447_545, 835_744, 814_578, 301_164_213]
    ),
    pair!(
        "pl",
        "uk",
        [1_394, 2_560_781, 850_461, 1_029_149, 306_987_789]
    ),
    pair!(
        "tr",
        "en",
        [1_381, 1_563_964, 839_750, 796_647, 306_727_185]
    ),
    pair!(
        "tr",
        "fr",
        [1_381, 1_512_671, 841_064, 814_377, 301_474_065]
    ),
    pair!(
        "tr",
        "es",
        [1_381, 1_501_501, 840_025, 816_299, 300_932_337]
    ),
    pair!(
        "tr",
        "uk",
        [1_394, 2_616_593, 859_415, 1_028_184, 307_461_801]
    ),
];

pub fn is_language(code: &str) -> bool {
    LANGUAGES.contains(&code)
}

pub fn is_target(code: &str) -> bool {
    TARGETS.contains(&code)
}

/// The direct model `source` → `target`, if there is one.
pub fn pair(source: &str, target: &str) -> Option<&'static PairSpec> {
    CATALOG
        .iter()
        .find(|p| p.source == source && p.target == target)
}

/// Models to chain for `source` → `target`: the direct one, or two through
/// English. `None` when the languages are the same or no route exists.
pub fn route(source: &str, target: &str) -> Option<Vec<&'static PairSpec>> {
    if source == target {
        return None;
    }
    if let Some(p) = pair(source, target) {
        return Some(vec![p]);
    }
    if source == PIVOT || target == PIVOT {
        return None;
    }
    Some(vec![pair(source, PIVOT)?, pair(PIVOT, target)?])
}

/// The models these settings need (each once).
pub fn needed(target: &str, sources: &[String]) -> Vec<&'static PairSpec> {
    let mut out: Vec<&'static PairSpec> = Vec::new();
    for s in sources {
        for p in route(s, target).unwrap_or_default() {
            if !out.contains(&p) {
                out.push(p);
            }
        }
    }
    out
}

/// Languages "Répondre" can write into: the players' languages Crosstalk can
/// translate into, except the user's own.
pub fn reply_targets(me: &str, sources: &[String]) -> Vec<String> {
    sources
        .iter()
        .filter(|s| s.as_str() != me && is_target(s) && route(me, s).is_some())
        .cloned()
        .collect()
}

/// The models these settings need, "Répondre" included (each once).
pub fn needed_with_replies(target: &str, sources: &[String]) -> Vec<&'static PairSpec> {
    let mut out = needed(target, sources);
    for r in reply_targets(target, sources) {
        for p in route(target, &r).unwrap_or_default() {
            if !out.contains(&p) {
                out.push(p);
            }
        }
    }
    out
}

pub fn pair_dir(models_dir: &Path, pair: &PairSpec) -> PathBuf {
    models_dir.join(pair.dir)
}

/// The first missing or truncated file, if any.
pub fn missing_file(models_dir: &Path, pair: &PairSpec) -> Option<&'static str> {
    let dir = pair_dir(models_dir, pair);
    pair.files.iter().find_map(|&(name, bytes)| {
        let ok = std::fs::metadata(dir.join(name)).is_ok_and(|m| m.len() == bytes);
        (!ok).then_some(name)
    })
}

/// Every model of the route is installed.
pub fn route_installed(models_dir: &Path, source: &str, target: &str) -> bool {
    route(source, target).is_some_and(|r| r.iter().all(|p| missing_file(models_dir, p).is_none()))
}

pub fn size_mb(pairs: &[&PairSpec]) -> u32 {
    (pairs
        .iter()
        .flat_map(|p| p.files.iter().map(|f| f.1))
        .sum::<u64>()
        / 1_000_000) as u32
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PairInfo {
    pub source: String,
    pub target: String,
    /// Repositories of the route (two through English).
    pub repos: Vec<&'static str>,
    pub size_mb: u32,
    pub installed: bool,
}

/// One line per player language other than the user's own.
pub fn list(models_dir: &Path, target: &str, sources: &[String]) -> Vec<PairInfo> {
    sources
        .iter()
        .filter(|s| s.as_str() != target)
        .filter_map(|s| {
            let r = route(s, target)?;
            Some(PairInfo {
                source: s.clone(),
                target: target.to_string(),
                repos: r.iter().map(|p| p.repo).collect(),
                size_mb: size_mb(&r),
                installed: r.iter().all(|p| missing_file(models_dir, p).is_none()),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_truncated_file_is_reported_missing() {
        let root = std::env::temp_dir().join(format!("crosstalk-mt-{}", std::process::id()));
        let p = pair("en", "fr").unwrap();
        let dir = pair_dir(&root, p);
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(missing_file(&root, p), Some("config.json"));
        std::fs::write(dir.join("config.json"), vec![b' '; 1_416]).unwrap();
        assert_eq!(missing_file(&root, p), Some("vocab.json"));
        let sources = vec!["en".to_string()];
        assert!(!list(&root, "fr", &sources).iter().any(|i| i.installed));
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn catalog_is_consistent() {
        for p in CATALOG {
            assert!(is_language(p.source) && is_language(p.target), "{}", p.repo);
            assert_eq!(p.dir, p.repo.trim_start_matches("Helsinki-NLP/"));
            assert_eq!(
                p.repo,
                format!("Helsinki-NLP/opus-mt-{}-{}", p.source, p.target)
            );
            assert!(p.files[4].1 > 250_000_000);
        }
        let mut repos: Vec<_> = CATALOG.iter().map(|p| p.repo).collect();
        repos.sort();
        repos.dedup();
        assert_eq!(repos.len(), CATALOG.len());
    }

    #[test]
    fn every_language_reaches_every_target() {
        for s in LANGUAGES {
            for t in TARGETS {
                if s == t {
                    assert!(route(s, t).is_none());
                    continue;
                }
                let r = route(s, t).unwrap_or_else(|| panic!("{s} → {t}"));
                assert!(!r.is_empty() && r.len() <= 2);
                assert_eq!(r[0].source, s);
                assert_eq!(r.last().unwrap().target, t);
            }
        }
    }

    #[test]
    fn missing_direct_pairs_go_through_english() {
        let r = route("de", "ru").unwrap();
        assert_eq!(
            r.iter().map(|p| p.repo).collect::<Vec<_>>(),
            ["Helsinki-NLP/opus-mt-de-en", "Helsinki-NLP/opus-mt-en-ru"]
        );
        assert_eq!(route("pl", "ru").unwrap().len(), 2);
        assert_eq!(route("ru", "fr").unwrap().len(), 1);
        // No model from English into Polish or Turkish.
        assert!(route("en", "pl").is_none() && !is_target("tr"));
    }

    #[test]
    fn needed_models_follow_the_settings() {
        let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        // Today's default: Russian and English voices into French.
        let def = needed("fr", &s(&["ru", "en"]));
        assert_eq!(
            def.iter().map(|p| p.dir).collect::<Vec<_>>(),
            ["opus-mt-ru-fr", "opus-mt-en-fr"]
        );
        // A Russian player: English and German voices into Russian; their own
        // language needs nothing, English is not downloaded twice.
        let ru = needed("ru", &s(&["en", "de", "ru"]));
        assert_eq!(
            ru.iter().map(|p| p.dir).collect::<Vec<_>>(),
            ["opus-mt-en-ru", "opus-mt-de-en"]
        );
        assert!(size_mb(&ru) > 600);
    }

    #[test]
    fn replies_go_from_the_users_language_to_the_players() {
        let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        assert_eq!(reply_targets("fr", &s(&["ru", "en"])), ["ru", "en"]);
        // Polish and Turkish players: no model writes into their language.
        assert_eq!(reply_targets("ru", &s(&["en", "pl", "ru"])), ["en"]);
        let all = needed_with_replies("fr", &s(&["ru", "en"]));
        let dirs: Vec<_> = all.iter().map(|p| p.dir).collect();
        assert_eq!(
            dirs,
            [
                "opus-mt-ru-fr",
                "opus-mt-en-fr",
                "opus-mt-fr-ru",
                "opus-mt-fr-en"
            ]
        );
    }
}
