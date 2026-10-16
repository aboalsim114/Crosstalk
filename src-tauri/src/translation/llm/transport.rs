//! HTTP for the LLM: one JSON POST (translation) or GET (model lists).
//! Abstract so that tests never reach the network.

use std::time::Duration;

use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HttpError {
    Status(u16),
    Timeout,
    Network(String),
}

pub trait Transport: Send + Sync {
    fn post(
        &self,
        url: &str,
        headers: &[(&str, String)],
        body: &Value,
        timeout: Duration,
    ) -> Result<String, HttpError>;
    fn get(
        &self,
        url: &str,
        headers: &[(&str, String)],
        timeout: Duration,
    ) -> Result<String, HttpError>;
}

pub struct UreqTransport;

fn agent(timeout: Duration) -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(timeout.min(Duration::from_secs(3)))
        .timeout(timeout)
        // A redirect could carry the key to another host: never followed.
        .redirects(0)
        .user_agent(concat!("Crosstalk/", env!("CARGO_PKG_VERSION")))
        .build()
}

fn answer(r: Result<ureq::Response, ureq::Error>) -> Result<String, HttpError> {
    match r {
        // 3xx (redirect refused) is not a usable answer.
        Ok(resp) if resp.status() >= 300 => Err(HttpError::Status(resp.status())),
        Ok(resp) => resp.into_string().map_err(|e| {
            if e.kind() == std::io::ErrorKind::TimedOut {
                HttpError::Timeout
            } else {
                HttpError::Network(e.kind().to_string())
            }
        }),
        // The body is dropped: some providers echo part of the key in it.
        Err(ureq::Error::Status(code, _)) => Err(HttpError::Status(code)),
        Err(ureq::Error::Transport(t)) => {
            let timeout = matches!(t.kind(), ureq::ErrorKind::Io)
                && t.to_string().to_ascii_lowercase().contains("timed out");
            if timeout {
                Err(HttpError::Timeout)
            } else {
                Err(HttpError::Network(t.kind().to_string()))
            }
        }
    }
}

impl Transport for UreqTransport {
    fn post(
        &self,
        url: &str,
        headers: &[(&str, String)],
        body: &Value,
        timeout: Duration,
    ) -> Result<String, HttpError> {
        let mut req = agent(timeout)
            .post(url)
            .set("Content-Type", "application/json");
        for (k, v) in headers {
            req = req.set(k, v);
        }
        answer(req.send_string(&body.to_string()))
    }

    fn get(
        &self,
        url: &str,
        headers: &[(&str, String)],
        timeout: Duration,
    ) -> Result<String, HttpError> {
        let mut req = agent(timeout).get(url);
        for (k, v) in headers {
            req = req.set(k, v);
        }
        answer(req.call())
    }
}

/// Error messages for the UI, from the status code only.
pub fn describe(e: &HttpError, timeout: Duration) -> String {
    match e {
        HttpError::Timeout => format!("délai dépassé ({:.1} s)", timeout.as_secs_f32()),
        HttpError::Network(kind) => format!("réseau indisponible ({kind})"),
        HttpError::Status(301..=308) => "redirection refusée (vérifiez l'adresse)".into(),
        HttpError::Status(401 | 403) => "clé refusée par le fournisseur".into(),
        HttpError::Status(402) => "crédit épuisé chez le fournisseur (402)".into(),
        HttpError::Status(404) => "modèle ou adresse introuvable (404)".into(),
        HttpError::Status(429) => "quota ou limite de débit atteint (429)".into(),
        HttpError::Status(code) if *code >= 500 => format!("erreur du fournisseur ({code})"),
        HttpError::Status(code) => format!("requête refusée ({code})"),
    }
}
