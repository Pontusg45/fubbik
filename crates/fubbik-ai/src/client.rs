//! The Ollama HTTP client.
//!
//! Ports `packages/api/src/ollama/client.ts`. Two deliberate differences
//! from that file:
//!
//! - The base URL is a field, not a module-level constant read from
//!   `OLLAMA_URL` at call time. Node's constant is why its only Ollama test
//!   can assert nothing but the degraded path; a field lets each test point
//!   at its own mock server.
//! - `generate_json` reports a decode failure distinctly from a transport
//!   failure. Node collapses both into one `AiError`; separating them costs
//!   nothing and makes the tests specific about which contract broke.
use serde::de::DeserializeOwned;

use crate::error::AiError;

/// Node's `EMBED_MODEL` (`ollama/client.ts:7`).
const EMBED_MODEL: &str = "nomic-embed-text";

/// Node's `isOllamaAvailable` uses a 2s `AbortSignal.timeout`
/// (`ollama/client.ts:20`).
const AVAILABILITY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);
const CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);
const GENERATE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);
const EMBED_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

#[derive(Debug, Clone)]
pub struct OllamaClient {
    base_url: String,
    http: reqwest::Client,
}

#[derive(serde::Deserialize)]
struct GenerateResponse {
    response: String,
}

#[derive(serde::Deserialize)]
struct EmbeddingResponse {
    embedding: Vec<f32>,
}

impl OllamaClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            // Trailing slashes would produce `//api/embeddings`, which
            // Ollama 404s on.
            base_url: base_url.into().trim_end_matches('/').to_string(),
            http: reqwest::Client::builder()
                .connect_timeout(CONNECT_TIMEOUT)
                .build()
                .expect("reqwest client configuration is valid"),
        }
    }

    /// Resolved once, at startup. Matches Node's
    /// `env.OLLAMA_URL ?? "http://localhost:11434"`.
    pub fn from_env() -> Self {
        Self::new(
            std::env::var("OLLAMA_URL").unwrap_or_else(|_| "http://localhost:11434".to_string()),
        )
    }

    /// Never errors — an unreachable Ollama is an expected state, not a
    /// fault. Callers branch on the bool exactly as Node does.
    pub async fn is_available(&self) -> bool {
        self.http
            .get(format!("{}/api/tags", self.base_url))
            .timeout(AVAILABILITY_TIMEOUT)
            .send()
            .await
            .map(|res| res.status().is_success())
            .unwrap_or(false)
    }

    pub async fn generate_json<T: DeserializeOwned>(
        &self,
        prompt: &str,
        model: &str,
    ) -> Result<T, AiError> {
        let res = self
            .http
            .post(format!("{}/api/generate", self.base_url))
            .json(&serde_json::json!({
                "model": model,
                "prompt": prompt,
                "format": "json",
                "stream": false,
            }))
            .timeout(GENERATE_TIMEOUT)
            .send()
            .await
            .map_err(|e| AiError::Transport(e.to_string()))?;

        if !res.status().is_success() {
            return Err(AiError::Status(res.status().as_u16()));
        }

        let body: GenerateResponse = res.json().await.map_err(|_| AiError::Decode)?;
        // Ollama returns the model's answer as a *string* that itself holds
        // JSON, so this is a second parse, not a nested field access.
        serde_json::from_str(&body.response).map_err(|_| AiError::Decode)
    }

    /// The model's answer as a raw string, without asking Ollama for JSON
    /// mode. Callers that must tolerate prose around the payload use this;
    /// callers that want strict JSON use [`Self::generate_json`].
    pub async fn generate_raw(&self, prompt: &str, model: &str) -> Result<String, AiError> {
        let res = self
            .http
            .post(format!("{}/api/generate", self.base_url))
            .json(&serde_json::json!({
                "model": model,
                "prompt": prompt,
                "stream": false,
            }))
            .timeout(GENERATE_TIMEOUT)
            .send()
            .await
            .map_err(|e| AiError::Transport(e.to_string()))?;

        if !res.status().is_success() {
            return Err(AiError::Status(res.status().as_u16()));
        }

        let body: GenerateResponse = res.json().await.map_err(|_| AiError::Decode)?;
        Ok(body.response)
    }

    pub async fn embed(&self, text: &str) -> Result<Vec<f32>, AiError> {
        let res = self
            .http
            .post(format!("{}/api/embeddings", self.base_url))
            .json(&serde_json::json!({ "model": EMBED_MODEL, "prompt": text }))
            .timeout(EMBED_TIMEOUT)
            .send()
            .await
            .map_err(|e| AiError::Transport(e.to_string()))?;

        if !res.status().is_success() {
            return Err(AiError::Status(res.status().as_u16()));
        }

        let body: EmbeddingResponse = res.json().await.map_err(|_| AiError::Decode)?;
        Ok(body.embedding)
    }

    /// `search_query: ` prefix per `ollama/client.ts:66`.
    pub async fn embed_query(&self, query: &str) -> Result<Vec<f32>, AiError> {
        self.embed(&format!("search_query: {query}")).await
    }

    /// `search_document: ` prefix per `ollama/client.ts:70-72`. The blank
    /// line for a missing summary is Node's `${summary ?? ""}` and is
    /// preserved: changing it changes every document vector.
    pub async fn embed_document(
        &self,
        title: &str,
        summary: Option<&str>,
        content: &str,
    ) -> Result<Vec<f32>, AiError> {
        let text = format!(
            "search_document: {title}\n{}\n{content}",
            summary.unwrap_or("")
        );
        self.embed(text.trim()).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    /// Captures the JSON body the client actually sent, so the prefix
    /// assertions below test the wire, not our own helper.
    async fn embed_capturing(body: serde_json::Value) -> serde_json::Value {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/embeddings"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "embedding": [0.0f32, 0.0f32, 0.0f32]
            })))
            .mount(&server)
            .await;
        let client = OllamaClient::new(server.uri());
        let _ = match body["kind"].as_str().unwrap() {
            "query" => client.embed_query(body["q"].as_str().unwrap()).await,
            _ => {
                client
                    .embed_document(
                        body["title"].as_str().unwrap(),
                        body["summary"].as_str(),
                        body["content"].as_str().unwrap(),
                    )
                    .await
            }
        };
        let requests = server.received_requests().await.unwrap();
        serde_json::from_slice(&requests[0].body).unwrap()
    }

    #[tokio::test]
    async fn embed_returns_the_vector_on_success() {
        // Given
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/embeddings"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "embedding": [0.25, -1.5, 3.0]
            })))
            .mount(&server)
            .await;

        // When
        let got = OllamaClient::new(server.uri())
            .embed("hello")
            .await
            .unwrap();
        // Then
        assert_eq!(got, vec![0.25, -1.5, 3.0]);
    }

    #[tokio::test]
    async fn embed_maps_a_non_2xx_to_status() {
        // Given
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/embeddings"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&server)
            .await;

        // When
        let err = OllamaClient::new(server.uri())
            .embed("hello")
            .await
            .unwrap_err();
        // Then
        assert!(matches!(err, AiError::Status(503)), "got {err:?}");
    }

    #[tokio::test]
    async fn embed_maps_an_undecodable_body_to_decode() {
        // Given
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/embeddings"))
            .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
            .mount(&server)
            .await;

        // When
        let err = OllamaClient::new(server.uri())
            .embed("hello")
            .await
            .unwrap_err();
        // Then
        assert!(matches!(err, AiError::Decode), "got {err:?}");
    }

    /// `nomic-embed-text` is asymmetric: a query embedded without the
    /// `search_query: ` prefix still returns a plausible vector, so nothing
    /// fails — every similarity score just quietly gets worse. This asserts
    /// the prefix reaches the wire.
    #[tokio::test]
    async fn embed_query_sends_the_search_query_prefix() {
        // Given the inline inputs and test fixtures.
        // When
        let sent = embed_capturing(serde_json::json!({ "kind": "query", "q": "auth" })).await;
        // Then
        assert_eq!(sent["prompt"], "search_query: auth");
        assert_eq!(sent["model"], "nomic-embed-text");
    }

    #[tokio::test]
    async fn embed_document_builds_nodes_exact_string() {
        // Given the inline inputs and test fixtures.
        // When
        let sent = embed_capturing(serde_json::json!({
            "kind": "document",
            "title": "T",
            "summary": "S",
            "content": "C"
        }))
        .await;
        // Then
        assert_eq!(sent["prompt"], "search_document: T\nS\nC");
    }

    /// Node's template interpolates `summary ?? ""`, leaving an empty line,
    /// and then `.trim()`s the whole string. A `None` summary must produce
    /// `"search_document: T\n\nC"` — not `"search_document: T\nC"`.
    #[tokio::test]
    async fn embed_document_keeps_the_blank_line_when_summary_is_none() {
        // Given the inline inputs and test fixtures.
        // When
        let sent = embed_capturing(serde_json::json!({
            "kind": "document",
            "title": "T",
            "summary": null,
            "content": "C"
        }))
        .await;
        // Then
        assert_eq!(sent["prompt"], "search_document: T\n\nC");
    }

    #[derive(Debug, serde::Deserialize, PartialEq)]
    struct Meta {
        summary: String,
    }

    #[tokio::test]
    async fn generate_json_parses_the_nested_response_field() {
        // Given
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/generate"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "response": "{\"summary\":\"ok\"}"
            })))
            .mount(&server)
            .await;

        // When
        let got: Meta = OllamaClient::new(server.uri())
            .generate_json("p", "llama3.2")
            .await
            .unwrap();
        // Then
        assert_eq!(
            got,
            Meta {
                summary: "ok".into()
            }
        );
    }

    /// Ollama answers 200 with a `response` string that is itself invalid
    /// JSON often enough that this is the realistic failure, not a 500.
    #[tokio::test]
    async fn generate_json_maps_an_unparseable_inner_payload_to_decode() {
        // Given
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/generate"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "response": "Sure! Here you go: {oops"
            })))
            .mount(&server)
            .await;

        // When
        let err = OllamaClient::new(server.uri())
            .generate_json::<Meta>("p", "llama3.2")
            .await
            .unwrap_err();
        // Then
        assert!(matches!(err, AiError::Decode), "got {err:?}");
    }

    #[tokio::test]
    async fn generate_json_sends_format_json_and_stream_false() {
        // Given
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/generate"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "response": "{\"summary\":\"ok\"}"
            })))
            .mount(&server)
            .await;

        let _: Meta = OllamaClient::new(server.uri())
            .generate_json("the prompt", "llama3.2")
            .await
            .unwrap();

        // When
        let requests = server.received_requests().await.unwrap();
        let sent: serde_json::Value = serde_json::from_slice(&requests[0].body).unwrap();
        // Then
        assert_eq!(sent["format"], "json");
        assert_eq!(sent["stream"], false);
        assert_eq!(sent["model"], "llama3.2");
        assert_eq!(sent["prompt"], "the prompt");
    }

    #[tokio::test]
    async fn is_available_is_true_on_200() {
        // Given
        let server = MockServer::start().await;
        // When
        Mock::given(method("GET"))
            .and(path("/api/tags"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .mount(&server)
            .await;
        // Then
        assert!(OllamaClient::new(server.uri()).is_available().await);
    }

    /// Node checks `res.ok`, so a reachable-but-broken Ollama counts as
    /// unavailable — not just a refused connection.
    #[tokio::test]
    async fn is_available_is_false_on_500() {
        // Given
        let server = MockServer::start().await;
        // When
        Mock::given(method("GET"))
            .and(path("/api/tags"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;
        // Then
        assert!(!OllamaClient::new(server.uri()).is_available().await);
    }

    #[tokio::test]
    async fn is_available_is_false_when_nothing_is_listening() {
        // Given the inline inputs and test fixtures.
        // When the operation is evaluated by the assertion.
        // Then
        // Port 1 is reserved and never has a listener.
        assert!(!OllamaClient::new("http://127.0.0.1:1").is_available().await);
    }

    #[tokio::test]
    async fn generate_raw_returns_the_full_string_including_prose() {
        // Given
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/generate"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "response": "Sure! [1,2]"
            })))
            .mount(&server)
            .await;

        // When
        let got = OllamaClient::new(server.uri())
            .generate_raw("p", "llama3.2")
            .await
            .unwrap();
        // Then
        assert_eq!(got, "Sure! [1,2]");

        let requests = server.received_requests().await.unwrap();
        let sent: serde_json::Value = serde_json::from_slice(&requests[0].body).unwrap();
        assert!(
            sent.get("format").is_none(),
            "generate_raw must not request JSON mode, got {sent:?}"
        );
    }

    #[tokio::test]
    async fn generate_raw_maps_a_non_2xx_to_status() {
        // Given
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/generate"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&server)
            .await;

        // When
        let err = OllamaClient::new(server.uri())
            .generate_raw("p", "llama3.2")
            .await
            .unwrap_err();
        // Then
        assert!(matches!(err, AiError::Status(503)), "got {err:?}");
    }
}
