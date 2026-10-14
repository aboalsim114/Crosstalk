use super::worker::{run, Engines};
use super::*;
use std::path::Path;
use std::sync::mpsc::{channel, Receiver, Sender};

struct Upper(Arc<AtomicU64>, Duration);
impl Engine for Upper {
    fn translate(&mut self, text: &str) -> Result<String, String> {
        self.0.fetch_add(1, Ordering::SeqCst);
        std::thread::sleep(self.1);
        Ok(format!("FR:{text}"))
    }
}

struct Loader {
    installed: Vec<&'static str>,
    calls: Arc<AtomicU64>,
    delay: Duration,
}
impl EngineLoader for Loader {
    fn installed(&self, _: &Path, source: &str, _: &str) -> bool {
        self.installed.contains(&source)
    }
    fn load(&self, _: &Path, _: &str, _: &str) -> Result<Box<dyn Engine>, String> {
        Ok(Box::new(Upper(self.calls.clone(), self.delay)))
    }
}

struct Events(Mutex<Sender<Translation>>);
impl TranslationEvents for Events {
    fn status(&self, _: &TranslationStatus) {}
    fn result(&self, t: &Translation) {
        let _ = self.0.lock().unwrap().send(t.clone());
    }
}

fn service(
    installed: Vec<&'static str>,
    delay_ms: u64,
) -> (
    Arc<TranslationService>,
    Receiver<Translation>,
    Arc<AtomicU64>,
) {
    let (tx, rx) = channel();
    let calls = Arc::new(AtomicU64::new(0));
    let svc = TranslationService::start(
        Arc::new(Events(Mutex::new(tx))),
        Arc::new(Loader {
            installed,
            calls: calls.clone(),
            delay: Duration::from_millis(delay_ms),
        }),
        PathBuf::from("models"),
        TranslationConfig::default(),
    );
    let end = Instant::now() + Duration::from_secs(5);
    while svc
        .status()
        .pairs
        .iter()
        .any(|p| p.state == PairState::Loading)
        || svc.inner.reload.load(Ordering::SeqCst)
    {
        assert!(Instant::now() < end);
        std::thread::sleep(Duration::from_millis(5));
    }
    (svc, rx, calls)
}

fn next(rx: &Receiver<Translation>) -> Translation {
    rx.recv_timeout(Duration::from_secs(5))
        .expect("translation event")
}

#[test]
fn known_callouts_need_no_model() {
    let (svc, rx, calls) = service(vec![], 0);
    svc.submit(1, "en", "CT, go, go.");
    let t = next(&rx);
    assert_eq!(t.translated.as_deref(), Some("CT, on y va"));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
fn subtitle_dashes_are_removed_from_the_model_output() {
    struct Dash;
    impl Engine for Dash {
        fn translate(&mut self, _: &str) -> Result<String, String> {
            Ok("- Deux vont sur B.".into())
        }
    }
    let mut engines: Engines = HashMap::new();
    engines.insert(("en".into(), "fr".into()), Box::new(Dash));
    let (svc, _rx, _) = service(vec![], 0);
    let job = Job {
        id: 1,
        lang: "en".into(),
        text: "Two are going B".into(),
        queued: Instant::now(),
        generation: 0,
        reply: None,
        partial: false,
    };
    let t = run(&svc.inner, &mut engines, &job).unwrap();
    assert_eq!(t.translated.as_deref(), Some("Deux vont sur B."));
}

#[test]
fn live_lines_wait_for_final_ones_and_only_the_latest_is_kept() {
    let (svc, rx, _) = service(vec!["ru"], 150);
    svc.submit(1, "ru", "первая фраза");
    svc.submit_partial(2, "ru", "живая");
    svc.submit_partial(2, "ru", "живая строка");
    svc.submit(3, "ru", "вторая фраза");
    let order: Vec<(u64, bool, String)> = (0..3)
        .map(|_| next(&rx))
        .map(|t| (t.id, t.partial, t.translated.unwrap()))
        .collect();
    assert_eq!(
        order,
        vec![
            (1, false, "FR:первая фраза".into()),
            (3, false, "FR:вторая фраза".into()),
            (2, true, "FR:живая строка".into())
        ]
    );
    assert!(
        rx.recv_timeout(Duration::from_millis(400)).is_err(),
        "the replaced live line was translated"
    );
}

#[test]
fn a_transcript_comes_back_in_french_then_from_the_cache() {
    let (svc, rx, calls) = service(vec!["ru", "en"], 0);
    assert!(svc
        .status()
        .pairs
        .iter()
        .all(|p| p.state == PairState::Ready));
    svc.submit(7, "ru", "Двое идут на B");
    let t = next(&rx);
    assert_eq!(
        (t.id, t.translated.as_deref(), t.cached),
        (7, Some("FR:Двое идут на B"), false)
    );
    svc.submit(8, "ru", "  двое   ИДУТ на b ");
    let t = next(&rx);
    assert_eq!(
        (t.id, t.translated.as_deref(), t.cached),
        (8, Some("FR:Двое идут на B"), true)
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn missing_model_or_disabled_translation_keeps_the_original() {
    let (svc, rx, _) = service(vec!["ru"], 0);
    let st = svc.status();
    assert_eq!(
        st.pairs.iter().find(|p| p.source == "en").unwrap().state,
        PairState::Missing
    );
    svc.submit(1, "en", "Two of them are going B");
    let t = next(&rx);
    assert_eq!(t.translated, None);
    assert_eq!(t.error.as_deref(), Some("modèle de traduction absent"));
    svc.configure(TranslationConfig {
        enabled: false,
        glossary: vec![],
        ..TranslationConfig::default()
    });
    svc.submit(2, "ru", "Двое");
    assert_eq!(next(&rx).error.as_deref(), Some("traduction désactivée"));
}

#[test]
fn french_is_never_sent_to_a_model() {
    let (svc, rx, calls) = service(vec!["ru", "en"], 0);
    svc.submit(3, "fr", "on push B");
    let t = next(&rx);
    assert_eq!(t.translated.as_deref(), Some("on push B"));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
fn purge_drops_queued_and_running_lines() {
    let (svc, rx, _) = service(vec!["ru"], 300);
    svc.submit(1, "ru", "раз");
    svc.submit(2, "ru", "два");
    std::thread::sleep(Duration::from_millis(50));
    svc.purge();
    assert!(rx.recv_timeout(Duration::from_millis(800)).is_err());
    svc.submit(3, "ru", "три");
    assert_eq!(next(&rx).id, 3);
}

#[test]
fn a_full_queue_reports_the_oldest_line_untranslated() {
    let (svc, rx, _) = service(vec!["ru"], 200);
    for id in 0..=QUEUE_CAPACITY as u64 + 1 {
        svc.submit(id, "ru", &format!("фраза {id}"));
    }
    let dropped: Vec<Translation> =
        std::iter::from_fn(|| rx.recv_timeout(Duration::from_millis(100)).ok())
            .filter(|t| t.error.as_deref() == Some("file pleine"))
            .collect();
    assert!(!dropped.is_empty());
    assert!(dropped.iter().all(|t| t.translated.is_none()));
}

#[test]
fn manual_test_waits_for_the_result_and_glossary_is_applied() {
    let (svc, _rx, _) = service(vec!["en"], 0);
    svc.configure(TranslationConfig {
        enabled: true,
        glossary: vec![Term {
            source: "smoke".into(),
            target: None,
        }],
        ..TranslationConfig::default()
    });
    let t = svc.translate_blocking("en", "throw Smoke");
    assert_eq!(t.translated.as_deref(), Some("FR:throw Smoke"));
    assert!(t.glossary);
}

#[test]
fn map_callouts_are_kept_and_the_user_glossary_wins() {
    let (svc, _rx, calls) = service(vec!["en"], 0);
    svc.configure(TranslationConfig {
        enabled: true,
        glossary: vec![Term {
            source: "palace".into(),
            target: Some("palais".into()),
        }],
        ..TranslationConfig::default()
    });
    svc.translate_blocking("en", "go jungle");
    svc.translate_blocking("en", "go jungle");
    let before = calls.load(Ordering::SeqCst);
    // A new map clears the cache: the line is translated again.
    svc.set_map_terms(vec!["Jungle".into(), "Palace".into()]);
    let t = svc.translate_blocking("en", "go jungle");
    assert_eq!(calls.load(Ordering::SeqCst), before + 1);
    assert_eq!(t.translated.as_deref(), Some("FR:go jungle"));
    let t = svc.translate_blocking("en", "push palace");
    assert_eq!(t.translated.as_deref(), Some("FR:push palais"));
    // Same map again: the cache is kept.
    svc.set_map_terms(vec!["Jungle".into(), "Palace".into()]);
    svc.translate_blocking("en", "go jungle");
    assert_eq!(calls.load(Ordering::SeqCst), before + 2);
}
fn settle(svc: &TranslationService) {
    let end = Instant::now() + Duration::from_secs(5);
    while svc
        .status()
        .pairs
        .iter()
        .any(|p| p.state == PairState::Loading)
        || svc.inner.reload.load(Ordering::SeqCst)
    {
        assert!(Instant::now() < end);
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn a_russian_user_gets_the_other_languages_in_russian() {
    let (svc, rx, calls) = service(vec!["en", "fr"], 0);
    let st = svc.configure(TranslationConfig {
        target: "ru".into(),
        sources: vec!["en".into(), "fr".into(), "ru".into(), "xx".into()],
        ..TranslationConfig::default()
    });
    assert_eq!(
        st.config.sources,
        ["en", "fr", "ru"],
        "unknown language dropped"
    );
    // Both models are installed: a pair still Missing has not been
    // looked at yet by the worker (it clears `reload` before the first
    // pair is marked Loading), so wait for it too.
    let end = Instant::now() + Duration::from_secs(5);
    let st = loop {
        settle(&svc);
        let st = svc.status();
        if st.pairs.iter().all(|p| p.state != PairState::Missing) || Instant::now() > end {
            break st;
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    let pairs: Vec<_> = st
        .pairs
        .iter()
        .map(|p| (p.source.as_str(), p.target.as_str(), p.state))
        .collect();
    assert_eq!(
        pairs,
        [
            ("en", "ru", PairState::Ready),
            ("fr", "ru", PairState::Ready)
        ]
    );
    // Their own language: shown as is.
    svc.submit(1, "ru", "Двое на B");
    let t = next(&rx);
    assert_eq!(
        (t.translated.as_deref(), t.target.as_str()),
        (Some("Двое на B"), "ru")
    );
    // The French callout lexicon does not answer into Russian: the model does.
    svc.submit(2, "en", "rush B");
    let t = next(&rx);
    assert_eq!(
        (t.translated.as_deref(), t.target.as_str()),
        (Some("FR:rush B"), "ru")
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    // A language not chosen stays as is.
    svc.submit(3, "de", "zwei auf B");
    assert_eq!(
        next(&rx).error.as_deref(),
        Some("langue non choisie (Langues)")
    );
}

#[test]
fn changing_the_target_reloads_and_never_mixes_cached_lines() {
    let (svc, rx, calls) = service(vec!["en", "ru", "fr"], 0);
    svc.submit(1, "en", "they rotate to B with the AWP");
    assert!(next(&rx).translated.is_some());
    svc.configure(TranslationConfig {
        target: "de".into(),
        sources: vec!["en".into()],
        ..TranslationConfig::default()
    });
    settle(&svc);
    svc.submit(2, "en", "they rotate to B with the AWP");
    let t = next(&rx);
    assert!(
        !t.cached,
        "a cached French line must not come back in German"
    );
    assert_eq!(t.target, "de");
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    // An unsupported target falls back to French.
    let st = svc.configure(TranslationConfig {
        target: "pl".into(),
        ..TranslationConfig::default()
    });
    assert_eq!(st.config.target, "fr");
}
