//! No test reaches the network: a scripted transport records the requests.
//! `TEST_KEY` is a placeholder, not a real key.

use super::client::{redact, PARTIAL_INTERVAL};
use super::config::is_local;
use super::local::{parse_ollama_tags, probe_lmstudio, probe_ollama, LocalKind};
use super::prompt::{is_translategemma, ANTHROPIC_VERSION};
use super::*;
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const TEST_KEY: &str = "sk-test-placeholder-0000";

type Sent = Arc<Mutex<Vec<(String, Vec<(String, String)>, Value, Duration)>>>;

#[derive(Default)]
struct Scripted {
    replies: Mutex<VecDeque<Result<String, HttpError>>>,
    sent: Sent,
}

impl Scripted {
    fn record(
        &self,
        url: &str,
        headers: &[(&str, String)],
        body: Value,
        timeout: Duration,
    ) -> Result<String, HttpError> {
        self.sent.lock().unwrap().push((
            url.into(),
            headers
                .iter()
                .map(|(k, v)| (k.to_string(), v.clone()))
                .collect(),
            body,
            timeout,
        ));
        self.replies
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or(Err(HttpError::Network("no scripted reply".into())))
    }
}

impl Transport for Scripted {
    fn post(
        &self,
        url: &str,
        headers: &[(&str, String)],
        body: &Value,
        timeout: Duration,
    ) -> Result<String, HttpError> {
        self.record(url, headers, body.clone(), timeout)
    }
    fn get(
        &self,
        url: &str,
        headers: &[(&str, String)],
        timeout: Duration,
    ) -> Result<String, HttpError> {
        self.record(url, headers, Value::Null, timeout)
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

fn ollama(model: &str) -> LlmConfig {
    LlmConfig {
        enabled: true,
        provider: Provider::Openai,
        base_url: "http://localhost:11434/v1".into(),
        model: model.into(),
    }
}

fn calls(sent: &Sent) -> usize {
    sent.lock().unwrap().len()
}

// ---------- configuration ----------

#[test]
fn configuration_is_sanitized_and_checked() {
    let c = openai().sanitized();
    assert_eq!(c.base_url, "https://api.openai.com/v1");
    assert_eq!(c.model, "gpt-4o-mini");
    assert_eq!(c.problem(), None);
    assert_eq!((c.kind(), c.backend()), (Kind::Cloud, None));
    assert_eq!(
        anthropic().sanitized().base_url,
        "https://api.anthropic.com/v1"
    );
    let none = LlmConfig {
        model: " ".into(),
        ..openai()
    }
    .sanitized();
    assert_eq!(none.problem().as_deref(), Some("modèle non choisi"));
    // A key never travels in clear text over the Internet; a local server may use http.
    assert!(LlmConfig {
        base_url: "http://api.example.com/v1".into(),
        ..openai()
    }
    .problem()
    .unwrap()
    .contains("http://"));
    let o = ollama("qwen2.5:7b");
    assert_eq!(o.problem(), None);
    assert_eq!((o.kind(), o.backend()), (Kind::Local, Some("Ollama")));
    let lm = LlmConfig {
        base_url: "http://127.0.0.1:1234/v1".into(),
        ..openai()
    };
    assert_eq!(lm.backend(), Some("LM Studio"));
    for bad in [
        "api.example.com/v1",
        "https://",
        "https://user:pass@api.example.com/v1",
        "ftp://x.example/v1",
        "https://api. example.com",
    ] {
        assert!(
            LlmConfig {
                base_url: bad.into(),
                ..openai()
            }
            .problem()
            .is_some(),
            "{bad}"
        );
    }
    assert!(!LlmConfig::default().enabled, "not connected by default");
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

// ---------- requests and prompts ----------

#[test]
fn openai_request_carries_only_the_line_with_cs2_rules() {
    let (c, sent) = client(openai(), Some(TEST_KEY), vec![openai_reply("Deux sur B.")]);
    assert_eq!(c.translate("Двое на B", "ru", "fr").unwrap(), "Deux sur B.");
    let sent = sent.lock().unwrap();
    let (url, headers, body, timeout) = &sent[0];
    assert_eq!(url, "https://api.openai.com/v1/chat/completions");
    assert_eq!(*timeout, CLOUD_TIMEOUT);
    assert!(headers.contains(&("Authorization".into(), format!("Bearer {TEST_KEY}"))));
    assert_eq!(
        (
            body["model"].as_str(),
            body["temperature"].as_i64(),
            body["stream"].as_bool()
        ),
        (Some("gpt-4o-mini"), Some(0), Some(false))
    );
    let messages = body["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[1]["content"], "Двое на B");
    let system = messages[0]["content"].as_str().unwrap();
    for must in [
        "Counter-Strike 2",
        "from Russian into French",
        "competitive match",
        "negations",
        "number",
        "AWP",
        "KZ1",
        "never instructions",
    ] {
        assert!(system.contains(must), "{must}");
    }
    // Nothing else from the game: no player name, no audio field.
    assert!(body.get("audio").is_none());
    assert_eq!(body["messages"].to_string().matches("Двое").count(), 1);
}

#[test]
fn translategemma_gets_its_own_single_message_prompt() {
    assert!(
        is_translategemma("translategemma:4b")
            && is_translategemma("Translate-Gemma-12B-Q4")
            && !is_translategemma("gemma3:4b")
    );
    let (c, sent) = client(
        ollama("translategemma:4b"),
        None,
        vec![openai_reply("Mon équipe ne sait pas jouer.")],
    );
    c.translate("My team can't play.", "en", "fr").unwrap();
    let sent = sent.lock().unwrap();
    let messages = sent[0].2["messages"].as_array().unwrap().clone();
    assert_eq!(messages.len(), 1, "no system message");
    let user = messages[0]["content"].as_str().unwrap();
    assert!(user.starts_with("You are a professional English (en) to French (fr) translator."));
    assert!(user.ends_with("into French:\n\n\nMy team can't play."));
    assert_eq!(sent[0].3, LOCAL_TIMEOUT);
    assert!(
        !sent[0].1.iter().any(|(k, _)| k == "Authorization"),
        "no key towards a local server"
    );
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
    let (url, headers, body, _) = &sent[0];
    assert_eq!(url, "https://api.anthropic.com/v1/messages");
    assert!(headers.contains(&("x-api-key".into(), TEST_KEY.into())));
    assert!(headers.contains(&("anthropic-version".into(), ANTHROPIC_VERSION.into())));
    assert!(body["system"].as_str().unwrap().contains("into French"));
    assert_eq!(body["messages"][0]["content"], "Сейвим, эко раунд");
    assert!(
        body["max_tokens"].as_u64().unwrap() >= 1024,
        "room for thinking"
    );
    assert!(
        body.get("temperature").is_none(),
        "recent Claude models reject it"
    );
    drop(sent);
    // A refusal (HTTP 200, stop_reason "refusal") is not a translation.
    let refusal = json!({ "stop_reason": "refusal", "content": [] });
    let thinking_then_text = json!({ "stop_reason": "end_turn", "content": [ { "type": "thinking", "thinking": "" }, { "type": "text", "text": "gg, bonne partie." } ] });
    let (c, _) = client(
        anthropic(),
        Some(TEST_KEY),
        vec![Ok(refusal.to_string()), Ok(thinking_then_text.to_string())],
    );
    assert_eq!(
        c.translate("гг", "ru", "fr").unwrap_err(),
        "réponse illisible"
    );
    assert_eq!(
        c.translate("гг, хорошая игра", "ru", "fr").unwrap(),
        "gg, bonne partie."
    );
}

#[test]
fn every_openai_compatible_cloud_uses_the_same_client() {
    for (base, model) in [
        ("https://api.mistral.ai/v1", "mistral-small-latest"),
        ("https://api.groq.com/openai/v1", "llama-3.1-8b-instant"),
        ("https://openrouter.ai/api/v1", "openai/gpt-4o-mini"),
        ("https://ollama.com/v1", "gemma4:31b"),
    ] {
        let (c, sent) = client(
            LlmConfig {
                base_url: base.into(),
                model: model.into(),
                ..openai()
            },
            Some(TEST_KEY),
            vec![openai_reply("gg, bonne partie.")],
        );
        assert!(c.active());
        assert_eq!(
            c.translate("гг, хорошая игра", "ru", "fr").unwrap(),
            "gg, bonne partie."
        );
        let sent = sent.lock().unwrap();
        assert_eq!(sent[0].0, format!("{base}/chat/completions"));
        assert!(sent[0]
            .1
            .contains(&("Authorization".into(), format!("Bearer {TEST_KEY}"))));
        assert_eq!(c.status().kind, Kind::Cloud);
    }
}

// ---------- errors, pauses, counters ----------

#[test]
fn failures_are_counted_and_never_reveal_the_key() {
    let (c, _) = client(
        openai(),
        Some(TEST_KEY),
        vec![
            Err(HttpError::Timeout),
            Err(HttpError::Network("Dns".into())),
            Ok("not json".into()),
            Err(HttpError::Status(500)),
            openai_reply("Un en bas."),
        ],
    );
    let errors: Vec<String> = (0..4)
        .map(|_| c.translate("one low", "en", "fr").unwrap_err())
        .collect();
    assert!(errors[0].starts_with("délai dépassé (6.0 s)"));
    assert!(errors[1].starts_with("réseau indisponible"));
    assert_eq!(errors[2], "réponse illisible");
    assert_eq!(errors[3], "erreur du fournisseur (500)");
    assert_eq!(c.translate("one low", "en", "fr").unwrap(), "Un en bas.");
    let st = c.status();
    assert_eq!(
        (st.stats.calls, st.stats.failures, st.stats.translations),
        (5, 4, 1)
    );
    assert!(st.stats.last_latency_ms.is_some());
    assert_eq!(st.stats.last_error, None, "cleared by the success");
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
fn a_quota_error_pauses_calls_instead_of_sending_more_paid_requests() {
    let (c, sent) = client(openai(), Some(TEST_KEY), vec![Err(HttpError::Status(429))]);
    assert!(c
        .translate("two mid", "en", "fr")
        .unwrap_err()
        .contains("429"));
    let again = c.translate("rotate A", "en", "fr").unwrap_err();
    assert!(again.contains("nouvel essai dans"), "{again}");
    assert_eq!(calls(&sent), 1, "no request during the pause");
    let st = c.status();
    assert!(st.paused_s.is_some_and(|s| (1..=20).contains(&s)));
    assert!(st.problem.unwrap().contains("quota"));
    // Another provider or model starts afresh.
    c.configure(LlmConfig {
        model: "gpt-4.1-mini".into(),
        ..openai()
    });
    assert!(c.status().paused_s.is_none());
}

#[test]
fn a_refused_key_stops_calls_until_a_new_key_is_saved() {
    let (c, sent) = client(
        openai(),
        Some(TEST_KEY),
        vec![
            Err(HttpError::Status(401)),
            openai_reply("Il est derrière toi."),
        ],
    );
    assert_eq!(
        c.translate("He's behind you", "en", "fr").unwrap_err(),
        "clé refusée par le fournisseur"
    );
    assert!(c
        .translate("He's behind you", "en", "fr")
        .unwrap_err()
        .contains("vérifiez la clé"));
    assert_eq!(calls(&sent), 1);
    c.set_key("sk-test-new-key-0001").unwrap();
    assert_eq!(
        c.translate("He's behind you", "en", "fr").unwrap(),
        "Il est derrière toi."
    );
    // 402 (no credit) behaves the same.
    let (c, sent) = client(openai(), Some(TEST_KEY), vec![Err(HttpError::Status(402))]);
    assert!(c.translate("eco", "en", "fr").unwrap_err().contains("402"));
    assert!(c.translate("eco", "en", "fr").is_err());
    assert_eq!(calls(&sent), 1);
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
    // Regression: an answer starting with a multi-byte letter used to panic.
    assert_eq!(
        plausible("They planted the bomb.", "Ils ont posé la bombe.").unwrap(),
        "Ils ont posé la bombe."
    );
    assert_eq!(plausible("Rush B", "Ëh, rush B").unwrap(), "Ëh, rush B");
}

#[test]
fn activity_depends_on_connection_model_and_key() {
    let (off, sent) = client(
        LlmConfig {
            enabled: false,
            ..openai()
        },
        Some(TEST_KEY),
        vec![],
    );
    assert!(!off.active());
    assert_eq!(
        off.status().problem,
        None,
        "no problem shown while disconnected"
    );
    assert!(Translator::problem(&*off).unwrap().contains("aucun moteur"));
    let (no_key, _) = client(openai(), None, vec![]);
    assert!(!no_key.active());
    assert_eq!(
        no_key.status().problem.as_deref(),
        Some("clé API non enregistrée")
    );
    assert!(no_key.translate("gg", "en", "fr").is_err());
    let (local, _) = client(ollama("qwen2.5:7b"), None, vec![]);
    assert!(local.active(), "a local server needs no key");
    assert_eq!(Translator::problem(&*local), None);
    assert!(calls(&sent) == 0);
    assert_eq!(clean_key("  sk-abc \n").unwrap(), "sk-abc");
    assert!(clean_key("   ").is_err());
    assert!(clean_key("two words").is_err());
}

#[test]
fn live_lines_never_go_to_a_cloud_provider_and_are_throttled_locally() {
    let (cloud, sent) = client(openai(), Some(TEST_KEY), vec![]);
    assert!(cloud
        .translate_partial("two", "en", "fr")
        .unwrap_err()
        .contains("cloud"));
    assert_eq!(calls(&sent), 0, "no paid call for a live line");
    let (local, sent) = client(
        ollama("qwen2.5:7b"),
        None,
        vec![openai_reply("deux"), openai_reply("deux sur B")],
    );
    assert_eq!(local.translate_partial("two", "en", "fr").unwrap(), "deux");
    assert!(local
        .translate_partial("two on", "en", "fr")
        .unwrap_err()
        .contains("rapproché"));
    std::thread::sleep(PARTIAL_INTERVAL);
    assert_eq!(
        local.translate_partial("two on B", "en", "fr").unwrap(),
        "deux sur B"
    );
    assert_eq!(calls(&sent), 2);
}

#[test]
fn a_local_server_stopping_then_restarting_needs_no_reconfiguration() {
    let (c, _) = client(
        ollama("qwen2.5:7b"),
        None,
        vec![
            openai_reply("Attendez-moi au mid."),
            Err(HttpError::Network("ConnectionFailed".into())),
            openai_reply("Ils ont posé la bombe."),
        ],
    );
    assert_eq!(
        c.translate("Wait for me mid.", "en", "fr").unwrap(),
        "Attendez-moi au mid."
    );
    let down = c
        .translate("They planted the bomb.", "en", "fr")
        .unwrap_err();
    assert!(down.starts_with("réseau indisponible"), "{down}");
    assert_eq!(
        c.translate("They planted the bomb.", "en", "fr").unwrap(),
        "Ils ont posé la bombe."
    );
}

// ---------- tests from the interface ----------

#[test]
fn connection_and_translation_tests_report_success_or_a_clear_error() {
    // Works while disconnected: the setup is checked before connecting.
    let (c, sent) = client(
        LlmConfig {
            enabled: false,
            ..openai()
        },
        Some(TEST_KEY),
        vec![
            openai_reply("Deux joueurs arrivent sur B, deux sur B."),
            Err(HttpError::Status(401)),
        ],
    );
    let ok = c.test(TEST_SENTENCE, "ru", "fr");
    assert!(ok.ok, "{ok:?}");
    assert_eq!(
        ok.translated.as_deref(),
        Some("Deux joueurs arrivent sur B, deux sur B.")
    );
    assert!(sent.lock().unwrap()[0].2["messages"][0]["content"]
        .as_str()
        .unwrap()
        .contains("from Russian into French"));
    let refused = c.test("Mon équipe ne sait pas jouer.", "fr", "ru");
    assert!(!refused.ok);
    assert_eq!(
        refused.error.as_deref(),
        Some("clé refusée par le fournisseur")
    );
    assert!(!format!("{refused:?}").contains(TEST_KEY));
    assert_eq!(c.status().stats.calls, 0, "tests are not counted");
    // Nothing to send without a key (cloud address): no request.
    let (no_key, sent2) = client(openai(), None, vec![]);
    assert_eq!(
        no_key.test("gg", "en", "fr").error.as_deref(),
        Some("clé API non enregistrée")
    );
    assert_eq!(calls(&sent2), 0);
}

#[test]
fn model_lists_come_from_the_server_with_the_stored_key() {
    let list = json!({ "object": "list", "data": [ { "id": "gpt-4o-mini" }, { "id": "gpt-4.1-mini" }, { "id": "gpt-4o-mini" } ] }).to_string();
    let (c, sent) = client(
        openai(),
        Some(TEST_KEY),
        vec![Ok(list), Err(HttpError::Status(401))],
    );
    let m = c.list_models();
    assert_eq!(
        (m.models, m.error),
        (
            vec!["gpt-4.1-mini".to_string(), "gpt-4o-mini".to_string()],
            None
        )
    );
    assert_eq!(
        sent.lock().unwrap()[0].0,
        "https://api.openai.com/v1/models"
    );
    assert!(sent.lock().unwrap()[0]
        .1
        .contains(&("Authorization".into(), format!("Bearer {TEST_KEY}"))));
    let refused = c.list_models();
    assert_eq!(
        refused.error.as_deref(),
        Some("clé refusée par le fournisseur")
    );
    // Claude: same path, its own headers.
    let (c, sent) = client(
        anthropic(),
        Some(TEST_KEY),
        vec![Ok(
            json!({ "data": [ { "id": "claude-haiku-4-5" } ] }).to_string()
        )],
    );
    assert_eq!(c.list_models().models, vec!["claude-haiku-4-5"]);
    assert!(sent.lock().unwrap()[0]
        .1
        .contains(&("anthropic-version".into(), ANTHROPIC_VERSION.into())));
    // A cloud address without key: no request at all.
    let (c, sent) = client(openai(), None, vec![]);
    assert_eq!(
        c.list_models().error.as_deref(),
        Some("clé API non enregistrée")
    );
    assert_eq!(calls(&sent), 0);
    // A local server needs no key.
    let (c, sent) = client(
        LlmConfig {
            model: String::new(),
            ..ollama("")
        },
        None,
        vec![Ok(json!({ "data": [ { "id": "qwen2.5:7b" } ] }).to_string())],
    );
    assert_eq!(c.list_models().models, vec!["qwen2.5:7b"]);
    assert_eq!(
        sent.lock().unwrap()[0].0,
        "http://localhost:11434/v1/models"
    );
}

// ---------- local servers ----------

#[test]
fn ollama_and_lm_studio_are_detected_with_their_real_models() {
    let t = Scripted::default();
    t.replies.lock().unwrap().extend([
        Ok(r#"{"models":[{"name":"qwen2.5:7b","size":4683087332},{"name":"llama3.2:3b"},{"model":"gemma3:4b"},{"name":"qwen2.5:7b"}]}"#.to_string()),
        Ok(json!({ "data": [ { "id": "qwen2.5-7b-instruct" }, { "id": "text-embedding-nomic-embed-text-v1.5" } ] }).to_string()),
    ]);
    let o = probe_ollama(&t);
    assert_eq!(
        (o.kind, o.running, o.base_url.as_str()),
        (LocalKind::Ollama, true, "http://localhost:11434/v1")
    );
    assert_eq!(o.models, vec!["gemma3:4b", "llama3.2:3b", "qwen2.5:7b"]);
    let lm = probe_lmstudio(&t);
    assert_eq!(
        (lm.kind, lm.running, lm.base_url.as_str()),
        (LocalKind::Lmstudio, true, "http://localhost:1234/v1")
    );
    assert_eq!(
        lm.models,
        vec!["qwen2.5-7b-instruct"],
        "embedding models cannot translate"
    );
    let sent = t.sent.lock().unwrap();
    assert_eq!(sent[0].0, "http://localhost:11434/api/tags");
    assert_eq!(sent[1].0, "http://localhost:1234/v1/models");
    assert!(
        sent.iter().all(|s| s.1.is_empty()),
        "no key towards local servers"
    );
}

#[test]
fn absent_or_empty_local_servers_are_reported_as_such() {
    let t = Scripted::default();
    t.replies.lock().unwrap().extend([
        Err(HttpError::Network("ConnectionRefused".into())),
        Ok(r#"{"models":[]}"#.to_string()),
    ]);
    let absent = probe_ollama(&t);
    assert!(!absent.running && absent.models.is_empty());
    let empty = probe_ollama(&t);
    assert!(
        empty.running && empty.models.is_empty(),
        "running without any model"
    );
    assert!(parse_ollama_tags("not json").is_empty());
}

// ---------- security and misc ----------

/// Real Credential Manager calls (Windows only), on a test entry that is not
/// the app's: the user's stored key is never touched.
#[cfg(windows)]
#[test]
fn credential_manager_round_trip() {
    use super::keys::CredentialStore;
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

/// Every launch sends the unchanged (default) settings: this used to keep the
/// settings lock while reading the status, a deadlock on the main thread
/// ("Crosstalk ne répond pas" at start-up).
#[test]
fn configuring_the_same_settings_twice_does_not_deadlock() {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let c = LlmClient::new(Arc::new(MemoryKeyStore::default()), Box::new(UreqTransport));
        c.configure(LlmConfig::default());
        c.configure(LlmConfig::default());
        let st = c.configure(openai());
        c.configure(openai());
        let _ = tx.send(st.config.model);
    });
    let model = rx
        .recv_timeout(Duration::from_secs(5))
        .expect("configure() deadlocked when the settings did not change");
    assert_eq!(model, "gpt-4o-mini");
}

#[test]
fn key_pages_come_from_a_fixed_list() {
    assert_eq!(
        key_page("ollama-cloud"),
        Some("https://ollama.com/settings/keys")
    );
    assert_eq!(key_page("lmstudio-download"), Some("https://lmstudio.ai/"));
    assert!(KEY_PAGES.iter().all(|(_, u)| u.starts_with("https://")));
    assert_eq!(key_page("https://evil.example"), None);
    assert_eq!(key_page(""), None);
}

#[test]
fn redirects_are_refused_and_explained() {
    use super::transport::describe;
    assert_eq!(
        describe(&HttpError::Status(302), CLOUD_TIMEOUT),
        "redirection refusée (vérifiez l'adresse)"
    );
}

// ---------- linguistic corpus (simulated model) ----------

/// The answers below are written by hand, as a model would give them: these
/// tests check what Crosstalk does with the request and the answer, not the
/// quality of a real model (no provider is reachable from the tests).
const CORPUS: [(&str, &str, &str); 5] = [
    (
        "Mon équipe ne sait pas jouer.",
        "Моя команда не умеет играть.",
        "My team can't play.",
    ),
    (
        "Deux ennemis arrivent sur B.",
        "Два врага идут на B.",
        "Two enemies are coming to B.",
    ),
    (
        "Attendez-moi au mid.",
        "Ждите меня на миде.",
        "Wait for me at mid.",
    ),
    (
        "Il est derrière toi.",
        "Он позади тебя.",
        "He's behind you.",
    ),
    (
        "Ils ont posé la bombe.",
        "Они поставили бомбу.",
        "They planted the bomb.",
    ),
];

/// Meaning kept, checked loosely: each group needs one of its forms (stems,
/// lower case), so a rephrasing passes and a dropped negation, number or
/// position fails.
fn meaning_kept(sentence: &str, out: &str) -> Result<(), String> {
    let groups: &[&[&str]] = match sentence {
        "Mon équipe ne sait pas jouer." => &[
            &[
                "не ", "can't", "cannot", "can not", "don't", "doesn't", "ne ", "pas",
            ],
            &["команд", "team", "équipe"],
            &["игра", "play", "jou"],
        ],
        "Deux ennemis arrivent sur B." => &[
            &["два", "двое", "2", "two", "deux"],
            &["враг", "противник", "enem", "ennemi"],
            &["b", "б"],
        ],
        "Attendez-moi au mid." => &[&["жди", "wait", "attend"], &["мид", "mid"]],
        "Il est derrière toi." => &[&["позади", "за тобой", "сзади", "behind", "derrière"]],
        "Ils ont posé la bombe." => &[&["бомб", "bomb"], &["постав", "заложил", "plant", "pos"]],
        _ => return Err(format!("phrase hors corpus : {sentence}")),
    };
    let low = format!(" {} ", out.to_lowercase());
    for g in groups {
        if !g.iter().any(|w| low.contains(w)) {
            return Err(format!("« {out} » perd le sens de « {sentence} » ({g:?})"));
        }
    }
    if out.contains('\n') {
        return Err(format!("« {out} » tient sur plusieurs lignes"));
    }
    Ok(())
}

#[test]
fn corpus_french_to_russian_and_english_keeps_the_meaning() {
    for (to, pick) in [("ru", 1), ("en", 2)] {
        for config in [
            openai(),
            anthropic(),
            ollama("qwen2.5:7b"),
            ollama("translategemma:4b"),
        ] {
            let replies = CORPUS
                .iter()
                .map(|c| {
                    let answer = if pick == 1 { c.1 } else { c.2 };
                    if config.provider == Provider::Anthropic {
                        Ok(json!({ "content": [ { "type": "text", "text": answer } ], "stop_reason": "end_turn" }).to_string())
                    } else {
                        openai_reply(answer)
                    }
                })
                .collect();
            let model = config.model.clone();
            let (c, sent) = client(config, Some(TEST_KEY), replies);
            for (fr, ..) in CORPUS {
                let out = c
                    .translate(fr, "fr", to)
                    .unwrap_or_else(|e| panic!("{model} {fr}: {e}"));
                meaning_kept(fr, &out).unwrap();
            }
            // Each request carries the sentence once, and the language pair.
            let sent = sent.lock().unwrap();
            assert_eq!(sent.len(), CORPUS.len());
            for ((_, _, body, _), (fr, ..)) in sent.iter().zip(CORPUS) {
                let text = body.to_string();
                assert_eq!(text.matches(fr).count(), 1, "{model}: {text}");
                let target = if to == "ru" { "Russian" } else { "English" };
                assert!(
                    text.contains("French") && text.contains(target),
                    "{model}: {text}"
                );
            }
        }
    }
}

#[test]
fn corpus_russian_and_english_to_french_keeps_the_meaning() {
    for (from, pick) in [("ru", 1), ("en", 2)] {
        let replies = CORPUS.iter().map(|c| openai_reply(c.0)).collect();
        let (c, _) = client(ollama("qwen2.5:7b"), None, replies);
        for row in CORPUS {
            let line = if pick == 1 { row.1 } else { row.2 };
            let out = c.translate(line, from, "fr").unwrap();
            meaning_kept(row.0, &out).unwrap();
        }
    }
}

#[test]
fn the_meaning_check_catches_a_lost_negation_number_or_position() {
    assert!(meaning_kept("Mon équipe ne sait pas jouer.", "Моя команда умеет играть.").is_err());
    assert!(meaning_kept("Deux ennemis arrivent sur A.", "Два врага идут на A.").is_err());
    assert!(meaning_kept("Deux ennemis arrivent sur B.", "Враги идут на B.").is_err());
    assert!(meaning_kept("Attendez-moi au mid.", "Ждите меня.").is_err());
    assert!(meaning_kept("Il est derrière toi.", "Он позади тебя.\nПримечание: …").is_err());
    // Rephrasings pass.
    assert!(meaning_kept(
        "Mon équipe ne sait pas jouer.",
        "Моя команда вообще не умеет играть."
    )
    .is_ok());
    assert!(meaning_kept("Deux ennemis arrivent sur B.", "Двое противников на B.").is_ok());
}

#[test]
fn a_commented_answer_never_reaches_the_overlay() {
    let (c, _) = client(
        openai(),
        Some(TEST_KEY),
        vec![openai_reply(
            "Voici la traduction :\n« Моя команда не умеет играть. »\nNote : registre familier.",
        )],
    );
    assert!(c
        .translate("Mon équipe ne sait pas jouer.", "fr", "ru")
        .is_err());
}
