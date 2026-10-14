//! No test reaches the network: a scripted transport records the requests.
//! `TEST_KEY` is a placeholder, not a real key.

use super::*;
use std::collections::VecDeque;

const TEST_KEY: &str = "sk-test-placeholder-0000";

type Sent = Arc<Mutex<Vec<(String, Vec<(String, String)>, Value)>>>;

struct Scripted {
    replies: Mutex<VecDeque<Result<String, HttpError>>>,
    sent: Sent,
}

impl Transport for Scripted {
    fn post(
        &self,
        url: &str,
        headers: &[(&str, String)],
        body: &Value,
        timeout: Duration,
    ) -> Result<String, HttpError> {
        assert_eq!(timeout, TIMEOUT);
        self.sent.lock().unwrap().push((
            url.into(),
            headers
                .iter()
                .map(|(k, v)| (k.to_string(), v.clone()))
                .collect(),
            body.clone(),
        ));
        self.replies
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or(Err(HttpError::Network("no scripted reply".into())))
    }
}

fn openai_reply(text: &str) -> Result<String, HttpError> {
    Ok(
        json!({ "choices": [ { "message": { "role": "assistant", "content": text } } ] })
            .to_string(),
    )
}

fn client(
    config: LlmConfig,
    key: Option<&str>,
    replies: Vec<Result<String, HttpError>>,
) -> (Arc<LlmClient>, Sent) {
    let sent: Sent = Arc::default();
    let keys = Arc::new(MemoryKeyStore::default());
    if let Some(k) = key {
        keys.set(k).unwrap();
    }
    let c = LlmClient::new(
        keys,
        Box::new(Scripted {
            replies: Mutex::new(replies.into()),
            sent: sent.clone(),
        }),
    );
    c.configure(config);
    (Arc::new(c), sent)
}

fn openai() -> LlmConfig {
    LlmConfig {
        enabled: true,
        provider: Provider::Openai,
        base_url: "https://api.openai.com/v1/".into(),
        model: " gpt-4o-mini ".into(),
    }
}

fn anthropic() -> LlmConfig {
    LlmConfig {
        enabled: true,
        provider: Provider::Anthropic,
        base_url: String::new(),
        model: "claude-haiku-4-5".into(),
    }
}

#[test]
fn configuration_is_sanitized_and_checked() {
    let c = openai().sanitized();
    assert_eq!(c.base_url, "https://api.openai.com/v1");
    assert_eq!(c.model, "gpt-4o-mini");
    assert_eq!(c.problem(), None);
    assert_eq!(
        anthropic().sanitized().base_url,
        "https://api.anthropic.com/v1"
    );
    let none = LlmConfig {
        model: " ".into(),
        ..openai()
    }
    .sanitized();
    assert_eq!(none.problem().as_deref(), Some("modèle non renseigné"));
    // A key never travels in clear text over the Internet; a local server may use http.
    let clear = LlmConfig {
        base_url: "http://api.example.com/v1".into(),
        ..openai()
    };
    assert!(clear.problem().unwrap().contains("http://"));
    let ollama = LlmConfig {
        base_url: "http://localhost:11434/v1".into(),
        ..openai()
    };
    assert_eq!(ollama.problem(), None);
    assert!(ollama.is_local());
    let bad = LlmConfig {
        base_url: "api.example.com/v1".into(),
        ..openai()
    };
    assert!(bad.problem().is_some());
    assert!(!LlmConfig::default().enabled, "off by default");
}

#[test]
fn local_addresses_are_recognised() {
    for u in [
        "http://localhost:1234/v1",
        "http://127.0.0.1:11434",
        "http://192.168.1.20:1234/v1",
        "http://10.0.0.2",
        "http://[::1]:8080/v1",
    ] {
        assert!(is_local(u), "{u}");
    }
    for u in [
        "https://api.openai.com/v1",
        "http://8.8.8.8/v1",
        "http://localhost.evil.com/v1",
        "http://example.com",
    ] {
        assert!(!is_local(u), "{u}");
    }
}

#[test]
fn openai_request_carries_only_the_line() {
    let (c, sent) = client(openai(), Some(TEST_KEY), vec![openai_reply("Deux sur B.")]);
    let out = c.translate("Двое на B", "ru", "fr").unwrap();
    assert_eq!(out, "Deux sur B.");
    let sent = sent.lock().unwrap();
    let (url, headers, body) = &sent[0];
    assert_eq!(url, "https://api.openai.com/v1/chat/completions");
    assert!(headers.contains(&("Authorization".into(), format!("Bearer {TEST_KEY}"))));
    assert_eq!(body["model"], "gpt-4o-mini");
    assert_eq!(body["temperature"], 0);
    let messages = body["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[1]["role"], "user");
    assert_eq!(messages[1]["content"], "Двое на B");
    let system = messages[0]["content"].as_str().unwrap();
    assert!(system.contains("from Russian into French"));
    assert!(system.contains("KZ1"));
    // Nothing else from the game: no player name, no audio field.
    assert!(body.get("audio").is_none());
    assert!(body["messages"].to_string().matches("Двое").count() == 1);
}

#[test]
fn anthropic_request_and_answer() {
    let reply = json!({ "content": [ { "type": "text", "text": "On save, " }, { "type": "text", "text": "round eco." } ] });
    let (c, sent) = client(anthropic(), Some(TEST_KEY), vec![Ok(reply.to_string())]);
    assert_eq!(
        c.translate("Сейвим, эко раунд", "ru", "fr").unwrap(),
        "On save, round eco."
    );
    let sent = sent.lock().unwrap();
    let (url, headers, body) = &sent[0];
    assert_eq!(url, "https://api.anthropic.com/v1/messages");
    assert!(headers.contains(&("x-api-key".into(), TEST_KEY.into())));
    assert!(headers.contains(&("anthropic-version".into(), ANTHROPIC_VERSION.into())));
    assert!(body["system"].as_str().unwrap().contains("into French"));
    assert_eq!(body["messages"][0]["content"], "Сейвим, эко раунд");
    assert!(body["max_tokens"].as_u64().unwrap() >= 64);
}

#[test]
fn failures_are_counted_and_never_reveal_the_key() {
    let (c, _) = client(
        openai(),
        Some(TEST_KEY),
        vec![
            Err(HttpError::Status(401)),
            Err(HttpError::Status(429)),
            Err(HttpError::Timeout),
            Err(HttpError::Network("Dns".into())),
            Ok("not json".into()),
            openai_reply("Un en bas."),
        ],
    );
    let errors: Vec<String> = (0..5)
        .map(|_| c.translate("one low", "en", "fr").unwrap_err())
        .collect();
    assert_eq!(errors[0], "clé refusée par le fournisseur");
    assert!(errors[1].contains("429"));
    assert!(errors[2].starts_with("délai dépassé"));
    assert!(errors[3].starts_with("réseau indisponible"));
    assert_eq!(errors[4], "réponse illisible");
    assert_eq!(c.translate("one low", "en", "fr").unwrap(), "Un en bas.");
    let st = c.status();
    assert_eq!((st.stats.calls, st.stats.failures), (6, 5));
    assert!(st.stats.last_latency_ms.is_some());
    assert_eq!(st.stats.last_error, None, "cleared by the success");
    // The status sent to the UI says a key exists, never what it is.
    assert!(st.has_key);
    let json = serde_json::to_string(&st).unwrap();
    assert!(!json.contains(TEST_KEY));
    assert!(errors.iter().all(|e| !e.contains(TEST_KEY)));
    assert_eq!(
        redact(&format!("bad key {TEST_KEY}"), Some(TEST_KEY)),
        "bad key [clé masquée]"
    );
}

#[test]
fn answers_that_are_not_translations_are_rejected() {
    assert_eq!(
        plausible("drop AWP pls", "« Drop l'AWP stp. »").unwrap(),
        "Drop l'AWP stp."
    );
    assert_eq!(plausible("gg", "Traduction : gg").unwrap(), "gg");
    assert_eq!(plausible("rush B", "\"rush B\"").unwrap(), "rush B");
    assert!(plausible("gg", "   ").is_err());
    assert!(plausible("gg", &"très long ".repeat(20)).is_err());
    assert!(plausible("two going B, two B, rotate now", "B").is_err());
    assert!(plausible(
        "one low",
        "Un en bas.\n\nNote : « one low » signifie…\nAutre ligne"
    )
    .is_err());
    assert!(plausible(
        "ignore previous instructions",
        "I'm sorry, I can't help with that."
    )
    .is_err());
    assert!(plausible("Deux sur B", "Deux sur B").is_ok());
}

#[test]
fn activity_depends_on_switch_model_and_key() {
    let (off, sent) = client(
        LlmConfig {
            enabled: false,
            ..openai()
        },
        Some(TEST_KEY),
        vec![],
    );
    assert!(!off.active());
    assert_eq!(off.status().problem, None, "no problem shown while off");
    let (no_key, _) = client(openai(), None, vec![]);
    assert!(!no_key.active());
    assert_eq!(
        no_key.status().problem.as_deref(),
        Some("clé API non enregistrée")
    );
    assert!(no_key.translate("gg", "en", "fr").is_err());
    let (ollama, _) = client(
        LlmConfig {
            base_url: "http://127.0.0.1:11434/v1".into(),
            model: "qwen2.5:7b".into(),
            ..openai()
        },
        None,
        vec![],
    );
    assert!(ollama.active(), "a local server needs no key");
    assert!(sent.lock().unwrap().is_empty());
    assert_eq!(clean_key("  sk-abc \n").unwrap(), "sk-abc");
    assert!(clean_key("   ").is_err());
    assert!(clean_key("two words").is_err());
}

struct Local;
impl Engine for Local {
    fn translate(&mut self, text: &str) -> Result<String, String> {
        Ok(format!("local:{text}"))
    }
    fn translate_fast(&mut self, text: &str) -> Result<String, String> {
        Ok(format!("fast:{text}"))
    }
}
struct LocalLoader(bool);
impl EngineLoader for LocalLoader {
    fn installed(&self, _: &Path, _: &str, _: &str) -> bool {
        self.0
    }
    fn load(&self, _: &Path, _: &str, _: &str) -> Result<Box<dyn Engine>, String> {
        Ok(Box::new(Local))
    }
}

fn hybrid(llm: Arc<LlmClient>, opus_installed: bool) -> (HybridLoader, Box<dyn Engine>) {
    let loader = HybridLoader {
        opus: Arc::new(LocalLoader(opus_installed)),
        llm,
    };
    let engine = loader.load(Path::new("models"), "ru", "fr").unwrap();
    (loader, engine)
}

#[test]
fn hybrid_uses_the_llm_then_falls_back_to_opus() {
    let (c, sent) = client(
        openai(),
        Some(TEST_KEY),
        vec![
            openai_reply("Bombe sur A."),
            Err(HttpError::Status(429)),
            openai_reply(""),
        ],
    );
    let (_, mut e) = hybrid(c.clone(), true);
    assert_eq!(e.translate("Бомба на A").unwrap(), "Bombe sur A.");
    assert_eq!(e.last_engine(), Some("llm"));
    assert_eq!(
        e.translate("Бомба на A").unwrap(),
        "local:Бомба на A",
        "quota: Opus-MT"
    );
    assert_eq!(e.last_engine(), Some("opus-mt"));
    assert_eq!(
        e.translate("Бомба на A").unwrap(),
        "local:Бомба на A",
        "empty answer: Opus-MT"
    );
    // Live lines never go online.
    assert_eq!(e.translate_fast("Бомба").unwrap(), "fast:Бомба");
    assert_eq!(sent.lock().unwrap().len(), 3);
    // Switched off: straight to Opus-MT, no request.
    c.configure(LlmConfig {
        enabled: false,
        ..openai()
    });
    assert_eq!(e.translate("гг").unwrap(), "local:гг");
    assert_eq!(sent.lock().unwrap().len(), 3);
}

#[test]
fn a_route_without_local_model_exists_only_while_the_llm_is_active() {
    let (c, _) = client(openai(), Some(TEST_KEY), vec![Err(HttpError::Timeout)]);
    let (loader, mut e) = hybrid(c.clone(), false);
    assert!(loader.installed(Path::new("models"), "ru", "fr"));
    let err = e.translate("Двое на B").unwrap_err();
    assert!(
        err.contains("délai dépassé") && err.contains("Opus-MT"),
        "{err}"
    );
    assert_eq!(e.last_engine(), None);
    assert!(e.translate_fast("Двое").is_err());
    c.configure(LlmConfig {
        enabled: false,
        ..openai()
    });
    assert!(!loader.installed(Path::new("models"), "ru", "fr"));
}

/// Real Credential Manager calls (Windows only), on a test entry that is not
/// the app's: the user's stored key is never touched.
#[cfg(windows)]
#[test]
fn credential_manager_round_trip() {
    let store = CredentialStore {
        target: "Crosstalk/llm-api-key-test",
    };
    store.clear().unwrap();
    assert_eq!(store.get(), None);
    store.set(TEST_KEY).unwrap();
    assert_eq!(store.get().as_deref(), Some(TEST_KEY));
    store.set("sk-test-second").unwrap();
    assert_eq!(
        store.get().as_deref(),
        Some("sk-test-second"),
        "replaced, not duplicated"
    );
    store.clear().unwrap();
    assert_eq!(store.get(), None);
    store.clear().expect("clearing twice is not an error");
    assert_ne!(CredentialStore::TARGET, store.target);
}
