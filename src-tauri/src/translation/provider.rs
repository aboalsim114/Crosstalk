use super::{TranslationPlan, TranslationSource};
use std::{future::Future, pin::Pin};
use zeroize::Zeroizing;
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ProviderError {
    Transient,
    Authentication,
    Quota,
    Invalid,
    Unavailable,
}
impl ProviderError {
    pub fn message(self) -> &'static str {
        match self {
            Self::Authentication => "Clé DeepL refusée.",
            Self::Quota => "Quota DeepL épuisé.",
            Self::Invalid => "Requête de traduction refusée.",
            Self::Transient | Self::Unavailable => "Moteur indisponible.",
        }
    }
}
pub struct TranslationRequest {
    pub xml: String,
    pub source: TranslationSource,
    pub plan: TranslationPlan,
    pub key: Zeroizing<String>,
}
pub trait Translator: Send + Sync {
    fn translate(
        &self,
        request: TranslationRequest,
    ) -> Pin<Box<dyn Future<Output = Result<String, ProviderError>> + Send + '_>>;
}
pub struct DeepL {
    client: reqwest::Client,
}
impl DeepL {
    pub fn new() -> Result<Self, ProviderError> {
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(std::time::Duration::from_millis(2400))
            .connect_timeout(std::time::Duration::from_secs(2))
            .build()
            .map_err(|_| ProviderError::Unavailable)?;
        Ok(Self { client })
    }
    async fn request(&self, request: TranslationRequest) -> Result<String, ProviderError> {
        let url = match request.plan {
            TranslationPlan::Free => "https://api-free.deepl.com/v2/translate",
            TranslationPlan::Pro => "https://api.deepl.com/v2/translate",
        };
        self.send(url, request).await
    }
    async fn send(&self, url: &str, request: TranslationRequest) -> Result<String, ProviderError> {
        let authorization = Zeroizing::new(format!("DeepL-Auth-Key {}", request.key.as_str()));
        let mut header = reqwest::header::HeaderValue::from_str(&authorization)
            .map_err(|_| ProviderError::Authentication)?;
        header.set_sensitive(true);
        let mut response = self.client.post(url).header(reqwest::header::AUTHORIZATION, header).json(&serde_json::json!({
            "text": [request.xml], "source_lang": request.source.deepl(), "target_lang": "FR", "tag_handling": "xml", "ignore_tags": "ctg", "outline_detection": false, "non_splitting_tags": "ctg", "preserve_formatting": true
        })).send().await.map_err(|e| if e.is_timeout() || e.is_connect() || e.is_request() { ProviderError::Transient } else { ProviderError::Unavailable })?;
        let status = response.status().as_u16();
        if !(200..300).contains(&status) {
            return Err(match status {
                403 => ProviderError::Authentication,
                456 => ProviderError::Quota,
                429 | 500..=599 => ProviderError::Transient,
                400 => ProviderError::Invalid,
                _ => ProviderError::Unavailable,
            });
        }
        const MAX_BODY: usize = 128 * 1024;
        if response
            .content_length()
            .is_some_and(|n| n > MAX_BODY as u64)
        {
            return Err(ProviderError::Unavailable);
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| ProviderError::Transient)?
        {
            if bytes.len() + chunk.len() > MAX_BODY {
                return Err(ProviderError::Unavailable);
            }
            bytes.extend_from_slice(&chunk);
        }
        #[derive(serde::Deserialize)]
        struct Item {
            text: String,
        }
        #[derive(serde::Deserialize)]
        struct Response {
            translations: Vec<Item>,
        }
        let mut response: Response =
            serde_json::from_slice(&bytes).map_err(|_| ProviderError::Unavailable)?;
        if response.translations.len() != 1 {
            return Err(ProviderError::Unavailable);
        }
        Ok(response.translations.remove(0).text)
    }
}
impl Translator for DeepL {
    fn translate(
        &self,
        request: TranslationRequest,
    ) -> Pin<Box<dyn Future<Output = Result<String, ProviderError>> + Send + '_>> {
        Box::pin(self.request(request))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    fn server(status: u16, body: &'static str) -> (String, std::thread::JoinHandle<()>) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/v2/translate", listener.local_addr().unwrap());
        let thread = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(std::time::Duration::from_secs(3)))
                .unwrap();
            let mut bytes = Vec::new();
            loop {
                let mut block = [0u8; 4096];
                let count = socket.read(&mut block).unwrap();
                if count == 0 {
                    break;
                }
                bytes.extend_from_slice(&block[..count]);
                if let Some(end) = bytes.windows(4).position(|v| v == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&bytes[..end]);
                    let length: usize = headers
                        .lines()
                        .find_map(|line| {
                            line.to_ascii_lowercase()
                                .strip_prefix("content-length: ")
                                .and_then(|s| s.parse().ok())
                        })
                        .unwrap();
                    if bytes.len() >= end + 4 + length {
                        // Boolean assertions avoid printing request headers or keys on failure.
                        assert!(headers.starts_with("POST /v2/translate HTTP/1.1"));
                        assert!(headers.lines().any(|v| v.eq_ignore_ascii_case(
                            "authorization: DeepL-Auth-Key synthetic-test-key"
                        )));
                        let payload: serde_json::Value =
                            serde_json::from_slice(&bytes[end + 4..]).unwrap();
                        assert!(payload["source_lang"] == "EN");
                        assert!(payload["target_lang"] == "FR");
                        assert!(payload["ignore_tags"] == "ctg");
                        assert!(payload["tag_handling"] == "xml");
                        break;
                    }
                }
            }
            let redirect = if status == 302 {
                "Location: http://127.0.0.1:9/not-allowed\r\n"
            } else {
                ""
            };
            let response = format!("HTTP/1.1 {status} Test\r\nContent-Length: {}\r\n{redirect}Connection: close\r\nContent-Type: application/json\r\n\r\n{body}", body.len());
            socket.write_all(response.as_bytes()).unwrap();
        });
        (url, thread)
    }
    fn request() -> TranslationRequest {
        TranslationRequest {
            xml: "<ctroot>Hello</ctroot>".into(),
            source: TranslationSource::En,
            plan: TranslationPlan::Free,
            key: Zeroizing::new("synthetic-test-key".into()),
        }
    }
    #[tokio::test]
    async fn local_http_contract_and_success() {
        let (url, thread) = server(
            200,
            r#"{"translations":[{"text":"<ctroot>Bonjour</ctroot>"}]}"#,
        );
        let provider = DeepL::new().unwrap_or_else(|_| panic!("client creation failed"));
        assert!(provider
            .send(&url, request())
            .await
            .is_ok_and(|v| v == "<ctroot>Bonjour</ctroot>"));
        thread.join().unwrap();
    }
    #[tokio::test]
    async fn status_errors_are_sanitized_and_redirects_fail() {
        for (status, expected) in [
            (403, ProviderError::Authentication),
            (456, ProviderError::Quota),
            (400, ProviderError::Invalid),
            (429, ProviderError::Transient),
            (503, ProviderError::Transient),
            (302, ProviderError::Unavailable),
        ] {
            let (url, thread) = server(status, "private provider error text");
            let provider = DeepL::new().unwrap_or_else(|_| panic!("client creation failed"));
            assert!(provider
                .send(&url, request())
                .await
                .is_err_and(|v| v == expected));
            thread.join().unwrap();
        }
    }
    #[tokio::test]
    async fn malformed_or_multiple_results_fail() {
        for body in [
            "not-json",
            r#"{"translations":[]}"#,
            r#"{"translations":[{"text":"a"},{"text":"b"}]}"#,
        ] {
            let (url, thread) = server(200, body);
            let provider = DeepL::new().unwrap_or_else(|_| panic!("client creation failed"));
            assert!(provider.send(&url, request()).await.is_err());
            thread.join().unwrap();
        }
    }
}
