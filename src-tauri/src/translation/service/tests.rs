//! The service with a simulated LLM: no network, no key.

use super::worker::run;
use super::*;
use std::sync::mpsc::{channel, Receiver, Sender};

/// "FR:<text>" (or "RU:<text>"…) after `delay`; can be made unavailable.
struct FakeLlm {
    calls: Arc<AtomicU64>,
    delay: Duration,
    problem: Mutex<Option<String>>,
    /// Live lines refused (as towards a cloud provider).
    no_partial: bool,
    answer: Option<String>,
}

impl Translator for FakeLlm {
    fn problem(&self) -> Option<String> {
        self.problem.lock().unwrap().clone()
    }
    fn translate(&self, text: &str, _source: &str, target: &str) -> Result<String, String> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        std::thread::sleep(self.delay);
        Ok(self
            .answer
            .clone()
            .unwrap_or_else(|| format!("{}:{text}", target.to_uppercase())))
    }
    fn translate_partial(&self, text: &str, source: &str, target: &str) -> Result<String, String> {
        if self.no_partial {
            return Err("provisoire non envoyé (fournisseur cloud)".into());
        }
        self.translate(text, source, target)
    }
}

struct Events(Mutex<Sender<Translation>>);
impl TranslationEvents for Events {
    fn status(&self, _: &TranslationStatus) {}
    fn result(&self, t: &Translation) {
        let _ = self.0.lock().unwrap().send(t.clone());
    }
}

fn with(llm: FakeLlm) -> (Arc<TranslationService>, Receiver<Translation>, Arc<FakeLlm>) {
    let (tx, rx) = channel();
    let llm = Arc::new(llm);
    let svc = TranslationService::start(
        Arc::new(Events(Mutex::new(tx))),
        llm.clone(),
        TranslationConfig::default(),
    );
    (svc, rx, llm)
}

fn fake(delay_ms: u64) -> FakeLlm {
    FakeLlm {
        calls: Arc::new(AtomicU64::new(0)),
        delay: Duration::from_millis(delay_ms),
        problem: Mutex::new(None),
        no_partial: false,
        answer: None,
    }
}

fn service(delay_ms: u64) -> (Arc<TranslationService>, Receiver<Translation>, Arc<FakeLlm>) {
    with(fake(delay_ms))
}

fn calls(llm: &FakeLlm) -> u64 {
    llm.calls.load(Ordering::SeqCst)
}

fn next(rx: &Receiver<Translation>) -> Translation {
    rx.recv_timeout(Duration::from_secs(5))
        .expect("translation event")
}

#[test]
fn known_callouts_are_exact_without_calling_the_llm() {
    let (svc, rx, llm) = service(0);
    svc.submit(1, "en", "CT, go, go.");
    let t = next(&rx);
    assert_eq!(t.translated.as_deref(), Some("CT, on y va"));
    assert_eq!(t.engine.as_deref(), Some("lexicon"));
    assert_eq!(calls(&llm), 0);
}

#[test]
fn subtitle_dashes_are_removed_from_the_answer() {
    let (svc, _rx, _) = with(FakeLlm {
        answer: Some("- Deux vont sur B.".into()),
        ..fake(0)
    });
    let job = Job {
        id: 1,
        lang: "en".into(),
        text: "Two are going B".into(),
        queued: Instant::now(),
        generation: 0,
        reply: None,
        partial: false,
    };
    assert_eq!(
        run(&svc.inner, &job).unwrap().translated.as_deref(),
        Some("Deux vont sur B.")
    );
}

#[test]
fn a_line_comes_back_from_the_llm_then_from_the_cache() {
    let (svc, rx, llm) = service(0);
    assert!(svc.status().ready);
    svc.submit(7, "ru", "Двое идут на B");
    let t = next(&rx);
    assert_eq!(
        (t.id, t.translated.as_deref(), t.cached, t.engine.as_deref()),
        (7, Some("FR:Двое идут на B"), false, Some("llm"))
    );
    svc.submit(8, "ru", "  двое   ИДУТ на b ");
    let t = next(&rx);
    assert_eq!(
        (t.id, t.translated.as_deref(), t.cached),
        (8, Some("FR:Двое идут на B"), true)
    );
    assert_eq!(calls(&llm), 1);
}

#[test]
fn without_an_engine_the_line_stays_in_its_language_with_the_reason() {
    let (svc, rx, llm) = service(0);
    *llm.problem.lock().unwrap() =
        Some("aucun moteur de traduction connecté (Audio & IA › Traduction IA)".into());
    let st = svc.status();
    assert!(!st.ready);
    assert!(st.problem.unwrap().contains("aucun moteur"));
    svc.submit(1, "en", "Two of them are going B");
    let t = next(&rx);
    assert_eq!(t.translated, None);
    assert!(t.error.unwrap().contains("aucun moteur"));
    assert_eq!(calls(&llm), 0, "nothing sent, nothing invented");
    // Exact callouts still work.
    svc.submit(2, "en", "rush");
    assert_eq!(next(&rx).translated.as_deref(), Some("rush"));
    svc.configure(TranslationConfig {
        enabled: false,
        ..TranslationConfig::default()
    });
    svc.submit(3, "ru", "Двое");
    assert_eq!(next(&rx).error.as_deref(), Some("traduction désactivée"));
}

#[test]
fn llm_errors_are_shown_and_never_replaced_by_another_engine() {
    struct Failing;
    impl Translator for Failing {
        fn problem(&self) -> Option<String> {
            None
        }
        fn translate(&self, _: &str, _: &str, _: &str) -> Result<String, String> {
            Err("quota ou limite de débit atteint (429)".into())
        }
        fn translate_partial(&self, _: &str, _: &str, _: &str) -> Result<String, String> {
            Err("provisoire non envoyé (fournisseur cloud)".into())
        }
    }
    let (tx, rx) = channel();
    let svc = TranslationService::start(
        Arc::new(Events(Mutex::new(tx))),
        Arc::new(Failing),
        TranslationConfig::default(),
    );
    svc.submit(1, "ru", "они сегодня медленные");
    let t = next(&rx);
    assert_eq!(t.translated, None);
    assert_eq!(
        t.error.as_deref(),
        Some("LLM : quota ou limite de débit atteint (429)")
    );
    assert_eq!(t.engine, None);
    // A refused live line publishes nothing: the final line will say why.
    svc.submit_partial(2, "ru", "живая");
    assert!(rx.recv_timeout(Duration::from_millis(300)).is_err());
}

#[test]
fn the_users_own_language_is_never_sent() {
    let (svc, rx, llm) = service(0);
    svc.submit(3, "fr", "on push B");
    assert_eq!(next(&rx).translated.as_deref(), Some("on push B"));
    assert_eq!(calls(&llm), 0);
}

#[test]
fn an_unsure_language_goes_to_the_llm_without_the_shortcuts() {
    let (svc, rx, llm) = service(0);
    // Maybe French, maybe not: never taken as the user's own, no lexicon.
    svc.submit(4, crate::translation::languages::UNSURE, "CT, go, go.");
    let t = next(&rx);
    assert_eq!(t.translated.as_deref(), Some("FR:CT, go, go."));
    assert_eq!(t.error, None);
    assert_eq!(calls(&llm), 1);
}

#[test]
fn live_lines_wait_for_final_ones_and_only_the_latest_is_kept() {
    let (svc, rx, _) = service(150);
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
fn live_lines_cost_nothing_when_the_engine_refuses_them() {
    let (svc, rx, llm) = with(FakeLlm {
        no_partial: true,
        ..fake(0)
    });
    for i in 0..5 {
        svc.submit_partial(1, "ru", &format!("живая строка {i}"));
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(rx.recv_timeout(Duration::from_millis(300)).is_err());
    assert_eq!(calls(&llm), 0);
    svc.submit(1, "ru", "живая строка");
    assert_eq!(next(&rx).translated.as_deref(), Some("FR:живая строка"));
    assert_eq!(calls(&llm), 1);
}

#[test]
fn purge_drops_queued_and_running_lines() {
    let (svc, rx, _) = service(300);
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
    let (svc, rx, _) = service(200);
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
    let (svc, _rx, _) = service(0);
    svc.configure(TranslationConfig {
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
    let (svc, _rx, llm) = service(0);
    svc.configure(TranslationConfig {
        glossary: vec![Term {
            source: "palace".into(),
            target: Some("palais".into()),
        }],
        ..TranslationConfig::default()
    });
    svc.translate_blocking("en", "go jungle");
    svc.translate_blocking("en", "go jungle");
    let before = calls(&llm);
    // A new map clears the cache: the line is translated again.
    svc.set_map_terms(vec!["Jungle".into(), "Palace".into()]);
    let t = svc.translate_blocking("en", "go jungle");
    assert_eq!(calls(&llm), before + 1);
    assert_eq!(t.translated.as_deref(), Some("FR:go jungle"));
    let t = svc.translate_blocking("en", "push palace");
    assert_eq!(t.translated.as_deref(), Some("FR:push palais"));
    svc.set_map_terms(vec!["Jungle".into(), "Palace".into()]);
    svc.translate_blocking("en", "go jungle");
    assert_eq!(calls(&llm), before + 2);
}

#[test]
fn a_russian_user_gets_every_other_language_in_russian_directly() {
    let (svc, rx, _) = service(0);
    svc.configure(TranslationConfig {
        target: "ru".into(),
        sources: vec!["en".into(), "fr".into(), "pl".into(), "ru".into()],
        ..TranslationConfig::default()
    });
    for (id, lang, text) in [
        (1, "en", "two on B"),
        (2, "fr", "deux en B"),
        (3, "pl", "dwóch na B"),
    ] {
        svc.submit(id, lang, text);
        let t = next(&rx);
        assert_eq!(
            (t.translated.as_deref(), t.target.as_str()),
            (Some(format!("RU:{text}").as_str()), "ru"),
            "{lang}"
        );
    }
    // French callouts of the lexicon are not used towards Russian.
    svc.submit(4, "en", "rush");
    assert_eq!(next(&rx).translated.as_deref(), Some("RU:rush"));
    // A language the user did not choose is shown as is.
    svc.submit(5, "de", "zwei auf B");
    assert_eq!(
        next(&rx).error.as_deref(),
        Some("langue non choisie (Langues)")
    );
    // Polish and Turkish can now be the user's language too.
    let st = svc.configure(TranslationConfig {
        target: "tr".into(),
        sources: vec!["en".into()],
        ..TranslationConfig::default()
    });
    assert_eq!(st.config.target, "tr");
}

#[test]
fn changing_the_target_never_mixes_cached_lines() {
    let (svc, _rx, llm) = service(0);
    assert_eq!(
        svc.translate_blocking("en", "they are slow today")
            .translated
            .as_deref(),
        Some("FR:they are slow today")
    );
    svc.configure(TranslationConfig {
        target: "de".into(),
        ..TranslationConfig::default()
    });
    assert_eq!(
        svc.translate_blocking("en", "they are slow today")
            .translated
            .as_deref(),
        Some("DE:they are slow today")
    );
    assert_eq!(calls(&llm), 2);
}

#[test]
fn a_new_llm_setup_clears_the_cache() {
    let (svc, _rx, llm) = service(0);
    svc.translate_blocking("ru", "они сегодня медленные");
    svc.translate_blocking("ru", "они сегодня медленные");
    assert_eq!(calls(&llm), 1);
    svc.engines_changed();
    svc.translate_blocking("ru", "они сегодня медленные");
    assert_eq!(calls(&llm), 2);
}

#[test]
fn a_line_waiting_too_long_keeps_its_language() {
    let (svc, _rx, llm) = service(0);
    let job = Job {
        id: 1,
        lang: "ru".into(),
        text: "Двое".into(),
        queued: Instant::now() - MAX_WAIT - Duration::from_millis(1),
        generation: 0,
        reply: None,
        partial: false,
    };
    let t = run(&svc.inner, &job).unwrap();
    assert_eq!(t.translated, None);
    assert!(t.error.unwrap().contains("délai dépassé"));
    assert_eq!(calls(&llm), 0);
}
