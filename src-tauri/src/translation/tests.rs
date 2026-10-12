use super::*;
use std::{
    future::Future,
    pin::Pin,
    sync::atomic::{AtomicUsize, Ordering},
};
#[derive(Default)]
struct Keys(Mutex<Option<Zeroizing<String>>>);
impl CredentialStore for Keys {
    fn supported(&self) -> bool {
        true
    }
    fn read(&self, _: TranslationPlan) -> Result<Option<Zeroizing<String>>, CredentialError> {
        Ok(self.0.lock().unwrap().clone())
    }
    fn write(&self, _: TranslationPlan, key: &str) -> Result<(), CredentialError> {
        *self.0.lock().unwrap() = Some(Zeroizing::new(key.into()));
        Ok(())
    }
    fn delete(&self, _: TranslationPlan) -> Result<(), CredentialError> {
        *self.0.lock().unwrap() = None;
        Ok(())
    }
}
struct Fake {
    calls: AtomicUsize,
    errors: Mutex<VecDeque<ProviderError>>,
    delay: Duration,
}
impl Translator for Fake {
    fn translate(
        &self,
        r: TranslationRequest,
    ) -> Pin<Box<dyn Future<Output = Result<String, ProviderError>> + Send + '_>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let e = self.errors.lock().unwrap().pop_front();
        Box::pin(async move {
            tokio::time::sleep(self.delay).await;
            if let Some(e) = e {
                Err(e)
            } else {
                Ok(r.xml)
            }
        })
    }
}
fn service(errors: Vec<ProviderError>, delay: Duration) -> (Arc<TranslationService>, Arc<Fake>) {
    let p = Arc::new(Fake {
        calls: AtomicUsize::new(0),
        errors: Mutex::new(errors.into()),
        delay,
    });
    (
        Arc::new(TranslationService::new(
            p.clone(),
            Arc::new(Keys::default()),
        )),
        p,
    )
}
fn enable(s: &TranslationService) {
    s.set_key(
        TranslationPlan::Free,
        Zeroizing::new("synthetic-test-key".into()),
    )
    .unwrap();
    s.configure(TranslationConfig {
        enabled: true,
        ..Default::default()
    })
    .unwrap();
}
#[tokio::test]
async fn skip_french_disabled_and_no_key() {
    let (s, p) = service(vec![], Duration::ZERO);
    assert!(s
        .translate("hello".into(), TranslationSource::En)
        .await
        .translated
        .is_none());
    s.configure(TranslationConfig {
        enabled: true,
        ..Default::default()
    })
    .unwrap();
    assert!(s
        .translate("hello".into(), TranslationSource::En)
        .await
        .translated
        .is_none());
    assert!(s
        .translate("bonjour".into(), TranslationSource::Fr)
        .await
        .reason
        .is_none());
    enable(&s);
    assert!(s
        .translate("bonjour".into(), TranslationSource::Fr)
        .await
        .translated
        .is_none());
    assert_eq!(p.calls.load(Ordering::SeqCst), 0);
}
#[tokio::test]
async fn key_never_enables_or_serializes() {
    let (s, p) = service(vec![], Duration::ZERO);
    let status = s
        .set_key(
            TranslationPlan::Free,
            Zeroizing::new("synthetic-test-key".into()),
        )
        .unwrap();
    assert!(!status.enabled);
    assert!(status.key_configured);
    assert!(!serde_json::to_string(&status)
        .unwrap()
        .contains("synthetic"));
    s.translate("hello".into(), TranslationSource::En).await;
    assert_eq!(p.calls.load(Ordering::SeqCst), 0);
}
#[tokio::test]
async fn lru_bounded_and_config_key_invalidates() {
    let (s, p) = service(vec![], Duration::ZERO);
    enable(&s);
    for i in 0..1001 {
        assert_eq!(
            s.translate(format!("text{i}"), TranslationSource::En)
                .await
                .status,
            "translated"
        );
    }
    assert_eq!(s.state.lock().unwrap().cache.len(), 1000);
    assert!(
        s.translate("text1".into(), TranslationSource::En)
            .await
            .cached
    );
    assert!(
        !s.translate("text0".into(), TranslationSource::En)
            .await
            .cached
    );
    assert_eq!(p.calls.load(Ordering::SeqCst), 1002);
    s.configure(TranslationConfig {
        enabled: true,
        glossary: vec![GlossaryEntry {
            source: "text1".into(),
            target: Some("imposé".into()),
        }],
        ..Default::default()
    })
    .unwrap();
    let r = s.translate("text1".into(), TranslationSource::En).await;
    assert!(!r.cached);
    assert_eq!(r.translated.as_deref(), Some("imposé"));
    s.set_key(TranslationPlan::Free, Zeroizing::new("new-test-key".into()))
        .unwrap();
    assert!(!s.status().unwrap().enabled);
    s.configure(TranslationConfig {
        enabled: true,
        glossary: vec![GlossaryEntry {
            source: "text1".into(),
            target: Some("imposé".into()),
        }],
        ..Default::default()
    })
    .unwrap();
    assert!(
        !s.translate("text1".into(), TranslationSource::En)
            .await
            .cached
    );
    s.delete_key(TranslationPlan::Free).unwrap();
    assert!(!s.status().unwrap().enabled);
    assert!(s.state.lock().unwrap().cache.is_empty());
}
#[tokio::test]
async fn transient_retry_once_other_errors_never_retry() {
    for (errors, calls) in [
        (vec![ProviderError::Transient], 2),
        (vec![ProviderError::Transient, ProviderError::Transient], 2),
        (vec![ProviderError::Authentication], 1),
        (vec![ProviderError::Quota], 1),
        (vec![ProviderError::Invalid], 1),
    ] {
        let (s, p) = service(errors, Duration::ZERO);
        enable(&s);
        let r = s.translate("hello".into(), TranslationSource::En).await;
        assert_eq!(p.calls.load(Ordering::SeqCst), calls);
        assert!(!r.reason.unwrap_or_default().contains("synthetic"));
    }
}
#[tokio::test]
async fn deadline_includes_retry_and_semaphore_wait() {
    let (s, p) = service(vec![], Duration::from_secs(10));
    enable(&s);
    let start = Instant::now();
    s.translate("hello".into(), TranslationSource::En).await;
    assert!(start.elapsed() < Duration::from_millis(5200));
    assert_eq!(p.calls.load(Ordering::SeqCst), 2);
    let permit = s.slots.acquire_many(2).await.unwrap();
    let start = Instant::now();
    s.translate("waiting".into(), TranslationSource::En).await;
    assert!(start.elapsed() < Duration::from_millis(5200));
    assert_eq!(p.calls.load(Ordering::SeqCst), 2);
    drop(permit);
}
#[tokio::test]
async fn disable_and_key_change_cancel_inflight() {
    for key_change in [false, true] {
        let (s, p) = service(vec![ProviderError::Transient], Duration::from_secs(10));
        enable(&s);
        let task = {
            let s = s.clone();
            tokio::spawn(async move { s.translate("hello".into(), TranslationSource::En).await })
        };
        while p.calls.load(Ordering::SeqCst) == 0 {
            tokio::task::yield_now().await;
        }
        if key_change {
            s.set_key(
                TranslationPlan::Free,
                Zeroizing::new("replacement-test-key".into()),
            )
            .unwrap();
        } else {
            s.configure(TranslationConfig::default()).unwrap();
        }
        assert!(task.await.unwrap().translated.is_none());
        assert!(s.state.lock().unwrap().cache.is_empty());
        assert_eq!(p.calls.load(Ordering::SeqCst), 1);
    }
}
#[tokio::test]
async fn invalid_input_zero_calls() {
    let (s, p) = service(vec![], Duration::ZERO);
    enable(&s);
    assert!(s
        .translate("x".repeat(8193), TranslationSource::En)
        .await
        .translated
        .is_none());
    assert_eq!(p.calls.load(Ordering::SeqCst), 0);
    assert!(s
        .configure(TranslationConfig {
            glossary: vec![GlossaryEntry {
                source: String::new(),
                target: None
            }],
            ..Default::default()
        })
        .is_err());
}

#[tokio::test]
async fn plan_change_and_failures_never_reuse_cache() {
    let (s, p) = service(vec![ProviderError::Quota], Duration::ZERO);
    enable(&s);
    assert!(s
        .translate("hello".into(), TranslationSource::En)
        .await
        .translated
        .is_none());
    assert!(
        !s.translate("hello".into(), TranslationSource::En)
            .await
            .cached
    );
    assert_eq!(p.calls.load(Ordering::SeqCst), 2);
    s.configure(TranslationConfig {
        enabled: true,
        plan: TranslationPlan::Pro,
        ..Default::default()
    })
    .unwrap();
    assert!(!s.status().unwrap().enabled);
    s.configure(TranslationConfig {
        enabled: true,
        plan: TranslationPlan::Pro,
        ..Default::default()
    })
    .unwrap();
    assert!(
        !s.translate("hello".into(), TranslationSource::En)
            .await
            .cached
    );
    assert_eq!(p.calls.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn unchanged_configuration_retains_cache() {
    let (s, p) = service(vec![], Duration::ZERO);
    enable(&s);
    s.translate("hello".into(), TranslationSource::En).await;
    s.configure(TranslationConfig {
        enabled: true,
        ..Default::default()
    })
    .unwrap();
    assert!(
        s.translate("hello".into(), TranslationSource::En)
            .await
            .cached
    );
    assert_eq!(p.calls.load(Ordering::SeqCst), 1);
}
#[tokio::test]
async fn bounded_requests_and_rendered_xml_never_reach_provider() {
    let (s, p) = service(vec![], Duration::ZERO);
    enable(&s);
    let held = s.pending.acquire_many(16).await.unwrap();
    assert!(s
        .translate("hello".into(), TranslationSource::En)
        .await
        .translated
        .is_none());
    drop(held);
    s.configure(TranslationConfig {
        enabled: true,
        glossary: vec![GlossaryEntry {
            source: ".".into(),
            target: None,
        }],
        ..Default::default()
    })
    .unwrap();
    assert!(s
        .translate(".".repeat(8192), TranslationSource::En)
        .await
        .translated
        .is_none());
    assert_eq!(p.calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn empty_provider_text_falls_back_and_is_not_cached() {
    struct Empty(AtomicUsize);
    impl Translator for Empty {
        fn translate(
            &self,
            _: TranslationRequest,
        ) -> Pin<Box<dyn Future<Output = Result<String, ProviderError>> + Send + '_>> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Box::pin(async { Ok("<ctroot/>".into()) })
        }
    }
    let provider = Arc::new(Empty(AtomicUsize::new(0)));
    let service = TranslationService::new(provider.clone(), Arc::new(Keys::default()));
    enable(&service);
    for _ in 0..2 {
        let result = service
            .translate("hello".into(), TranslationSource::En)
            .await;
        assert_eq!(result.status, "original");
        assert!(result.translated.is_none());
        assert!(!result.cached);
        assert_eq!(result.original, "hello");
    }
    assert_eq!(provider.0.load(Ordering::SeqCst), 2);
    assert!(service.state.lock().unwrap().cache.is_empty());
}
