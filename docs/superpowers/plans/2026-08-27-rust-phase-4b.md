# Rust Phase 4b — Ollama Core Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Port the Ollama-backed half of Phase 4 to Rust — a `fubbik-ai` transport crate, the embedding write path, and the five endpoints that depend on them — so that edited chunks stop keeping stale vectors and unified search's `similar-to` clause stops resolving to nothing.

**Architecture:** A new `fubbik-ai` crate owns Ollama's HTTP contract and nothing else (no prompts). Its `OllamaClient` is constructed once at startup and carried on `AppState`, so tests inject a `wiremock` base URL instead of relying on a real Ollama. Repository reads/writes for vectors reuse the existing text-cast approach in `fubbik-db/src/embedding.rs` rather than adding the `pgvector` crate. Domain logic and prompts stay in `fubbik-api` beside the routes that own them.

**Tech Stack:** Rust 2024, axum 0.8, sqlx 0.8 (offline `.sqlx` cache), utoipa 5, reqwest 0.12, wiremock 0.6, tokio, Postgres 18 + pgvector 0.8.2 + Apache AGE.

**Spec:** `docs/superpowers/specs/2026-08-27-rust-phase-4b-design.md`

## Global Constraints

- `export DATABASE_URL="postgres://postgres:password@localhost:5434/fubbik_rs"` before any `cargo` command that touches sqlx macros.
- The dev database runs in the repo's own image `fubbik-postgres:pg18-vector-age` (built by `docker/postgres/Dockerfile`, wired at `docker-compose.yml:68`). It is the only image that has **both** pgvector and AGE. Do not substitute a Docker Hub image.
- Every `docker` command needs `--context orbstack`. Never run `docker context use`.
- `cargo sqlx prepare --workspace` **alone drops test-target-only entries.** Always run `cargo sqlx prepare --workspace -- --tests`.
- CI runs `pgvector/pgvector:pg18` (`.github/workflows/rust.yml:17`): **pgvector is present, AGE is not.** Vector assertions run in CI; AGE assertions must be gated with `fubbik_db::age::is_available(&pool)` and skipped when absent, exactly as in Phase 4a.
- Commit with explicit pathspecs. Never `git add -A`.
- No `Co-Authored-By` or "Generated with Claude" trailers.
- Never run `git push`. The user asks for it explicitly or it does not happen.
- Do **not** start the Node server against the user's live knowledge base without asking.
- Every `#[sqlx::test]` needs `migrations = "../fubbik-db/migrations"`. All 40+ existing test files carry it; one without it silently runs against an unmigrated database.
- `cargo fmt` before every commit. Phase 4a lost a round to this.
- Node parity is the default. Where this plan diverges from Node it says so explicitly and why; anywhere it is silent, match Node.

---

## File Structure

**New crate `crates/fubbik-ai/`**
- `Cargo.toml` — depends on `fubbik-core`, `reqwest`, `serde`, `serde_json`, `thiserror`; dev-depends on `wiremock`, `tokio`.
- `src/lib.rs` — re-exports `OllamaClient`, `AiError`.
- `src/error.rs` — `AiError` and its `From<AiError> for AppError`.
- `src/client.rs` — `OllamaClient` and its six methods, plus wiremock unit tests.

**`crates/fubbik-db/src/`**
- `repo/semantic.rs` (new) — `semantic_search`, `find_neighbors_by_chunk_id`.
- `repo/similarity.rs` (new) — `find_similar_by_embedding`.
- `repo/chunk.rs` (modify) — add `update_chunk_enrichment`.
- `repo/mod.rs` (modify) — register both new modules.

**`crates/fubbik-api/src/`**
- `lib.rs` (modify) — `AppState` gains `ai` and `rate_limiter`; router merges `enrich::routes::router()`.
- `middleware/rate_limit.rs` (new) + `middleware/mod.rs` (new).
- `enrich/{mod,service,routes}.rs` (new) — the enrich domain and its prompt.
- `chunks/ai.rs` (new) — `semantic_search`, `check_similar`, `neighbors` service functions.
- `chunks/dto.rs` (modify) — query/body/response DTOs for the three new routes.
- `chunks/routes.rs` (modify) — three new handlers.
- `chunks/service.rs` — unchanged; the re-enrich hook goes in the route, see Task 10.
- `search/service.rs` (modify) — un-stub the `similar-to` branch.
- `search/routes.rs` (modify) — pass the client through.
- `vocabulary/suggest.rs` (modify) — refactor onto the client.

**`crates/fubbik/src/main.rs`** (modify) — build the client and limiter into `AppState`.

**`apps/web/src/`** — five call sites move from `legacyApi` to `api`; the doc block in `utils/api.ts` is updated.

---

## Task 0: Baseline

**Files:** none (measurement only)

**Interfaces:**
- Consumes: nothing
- Produces: a recorded baseline test count that every later task compares against

- [ ] **Step 1: Start the database**

```bash
docker --context orbstack compose up -d postgres
```

- [ ] **Step 2: Confirm both extensions are present**

```bash
export DATABASE_URL="postgres://postgres:password@localhost:5434/fubbik_rs"
psql "$DATABASE_URL" -c "SELECT extname FROM pg_extension ORDER BY extname;"
```

Expected: the list includes both `vector` and `age`. If either is missing, STOP and report — a missing extension poisons every measurement that follows.

- [ ] **Step 3: Record the baseline**

```bash
export DATABASE_URL="postgres://postgres:password@localhost:5434/fubbik_rs"
cargo test --workspace 2>&1 | tail -40
```

Expected: **1084 passed, 0 failed, 13 ignored** (the Phase 4a exit state). Write the actual numbers down. If they differ, report the difference before continuing — do not proceed against a baseline you cannot explain.

- [ ] **Step 4: Confirm the working tree is clean**

```bash
git status --short
```

Expected: empty output.

---

## Task 1: The `fubbik-ai` crate

**Files:**
- Create: `crates/fubbik-ai/Cargo.toml`
- Create: `crates/fubbik-ai/src/lib.rs`
- Create: `crates/fubbik-ai/src/error.rs`
- Create: `crates/fubbik-ai/src/client.rs` (implementation and tests in the same file, matching this repo's `#[cfg(test)] mod tests` convention)

**Interfaces:**
- Consumes: `fubbik_core::error::AppError`
- Produces:
  - `fubbik_ai::AiError`
  - `impl From<AiError> for AppError` (maps to `AppError::External`)
  - `fubbik_ai::OllamaClient` with:
    - `fn new(base_url: impl Into<String>) -> Self`
    - `fn from_env() -> Self`
    - `async fn is_available(&self) -> bool`
    - `async fn generate_json<T: serde::de::DeserializeOwned>(&self, prompt: &str, model: &str) -> Result<T, AiError>`
    - `async fn embed(&self, text: &str) -> Result<Vec<f32>, AiError>`
    - `async fn embed_query(&self, query: &str) -> Result<Vec<f32>, AiError>`
    - `async fn embed_document(&self, title: &str, summary: Option<&str>, content: &str) -> Result<Vec<f32>, AiError>`
  - `OllamaClient` must be `Clone` (it goes on `AppState`, which axum clones per request) — `reqwest::Client` is internally reference-counted, so deriving `Clone` is cheap and correct.

- [ ] **Step 1: Create the manifest**

`crates/fubbik-ai/Cargo.toml`:

```toml
[package]
name = "fubbik-ai"
edition.workspace = true
version.workspace = true

[dependencies]
reqwest = { version = "0.12", features = ["json"] }
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
fubbik-core = { path = "../fubbik-core" }

[dev-dependencies]
tokio.workspace = true
wiremock = "0.6"
```

The workspace's `members = ["crates/*"]` glob picks this up with no root `Cargo.toml` edit.

- [ ] **Step 2: Write the error type**

`crates/fubbik-ai/src/error.rs`:

```rust
//! `AiError` deliberately does not carry the upstream response body.
//!
//! Node logs the cause and returns a fixed `"AI service error"` string to
//! the client (`packages/api/src/index.ts:193-196`). Prompts and model
//! output can contain chunk content, so the variants below capture only
//! what is safe to surface: the failing status, or the fact that decoding
//! failed.
use fubbik_core::error::AppError;

#[derive(Debug, thiserror::Error)]
pub enum AiError {
    #[error("ollama request failed: {0}")]
    Transport(String),

    #[error("ollama returned status {0}")]
    Status(u16),

    #[error("ollama returned a body this client could not decode")]
    Decode,
}

/// Maps onto `AppError::External`, which `crates/fubbik-api/src/error.rs:36`
/// already renders as **502 Bad Gateway** — the same status Node's global
/// handler gives `AiError`.
impl From<AiError> for AppError {
    fn from(err: AiError) -> Self {
        AppError::External(err.to_string())
    }
}
```

- [ ] **Step 3: Write the lib root**

`crates/fubbik-ai/src/lib.rs`:

```rust
//! Ollama transport. Knows Ollama's HTTP contract and nothing about fubbik.
//!
//! No prompt text lives in this crate: prompts are domain knowledge and
//! belong beside the domain that owns them (`fubbik-api`'s `enrich` and
//! `vocabulary` modules each keep their own).
pub mod client;
pub mod error;

pub use client::OllamaClient;
pub use error::AiError;
```

- [ ] **Step 4: Write the failing tests**

`crates/fubbik-ai/src/client.rs` — put this `#[cfg(test)]` block at the bottom of the file you are about to create; write it before the implementation.

```rust
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
                "embedding": [0.0f32; 3]
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
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/embeddings"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "embedding": [0.25, -1.5, 3.0]
            })))
            .mount(&server)
            .await;

        let got = OllamaClient::new(server.uri()).embed("hello").await.unwrap();
        assert_eq!(got, vec![0.25, -1.5, 3.0]);
    }

    #[tokio::test]
    async fn embed_maps_a_non_2xx_to_status() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/embeddings"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&server)
            .await;

        let err = OllamaClient::new(server.uri()).embed("hello").await.unwrap_err();
        assert!(matches!(err, AiError::Status(503)), "got {err:?}");
    }

    #[tokio::test]
    async fn embed_maps_an_undecodable_body_to_decode() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/embeddings"))
            .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
            .mount(&server)
            .await;

        let err = OllamaClient::new(server.uri()).embed("hello").await.unwrap_err();
        assert!(matches!(err, AiError::Decode), "got {err:?}");
    }

    /// `nomic-embed-text` is asymmetric: a query embedded without the
    /// `search_query: ` prefix still returns a plausible vector, so nothing
    /// fails — every similarity score just quietly gets worse. This asserts
    /// the prefix reaches the wire.
    #[tokio::test]
    async fn embed_query_sends_the_search_query_prefix() {
        let sent = embed_capturing(serde_json::json!({ "kind": "query", "q": "auth" })).await;
        assert_eq!(sent["prompt"], "search_query: auth");
        assert_eq!(sent["model"], "nomic-embed-text");
    }

    #[tokio::test]
    async fn embed_document_builds_nodes_exact_string() {
        let sent = embed_capturing(serde_json::json!({
            "kind": "document",
            "title": "T",
            "summary": "S",
            "content": "C"
        }))
        .await;
        assert_eq!(sent["prompt"], "search_document: T\nS\nC");
    }

    /// Node's template interpolates `summary ?? ""`, leaving an empty line,
    /// and then `.trim()`s the whole string. A `None` summary must produce
    /// `"search_document: T\n\nC"` — not `"search_document: T\nC"`.
    #[tokio::test]
    async fn embed_document_keeps_the_blank_line_when_summary_is_none() {
        let sent = embed_capturing(serde_json::json!({
            "kind": "document",
            "title": "T",
            "summary": null,
            "content": "C"
        }))
        .await;
        assert_eq!(sent["prompt"], "search_document: T\n\nC");
    }

    #[derive(Debug, serde::Deserialize, PartialEq)]
    struct Meta {
        summary: String,
    }

    #[tokio::test]
    async fn generate_json_parses_the_nested_response_field() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/generate"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "response": "{\"summary\":\"ok\"}"
            })))
            .mount(&server)
            .await;

        let got: Meta = OllamaClient::new(server.uri())
            .generate_json("p", "llama3.2")
            .await
            .unwrap();
        assert_eq!(got, Meta { summary: "ok".into() });
    }

    /// Ollama answers 200 with a `response` string that is itself invalid
    /// JSON often enough that this is the realistic failure, not a 500.
    #[tokio::test]
    async fn generate_json_maps_an_unparseable_inner_payload_to_decode() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/generate"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "response": "Sure! Here you go: {oops"
            })))
            .mount(&server)
            .await;

        let err = OllamaClient::new(server.uri())
            .generate_json::<Meta>("p", "llama3.2")
            .await
            .unwrap_err();
        assert!(matches!(err, AiError::Decode), "got {err:?}");
    }

    #[tokio::test]
    async fn generate_json_sends_format_json_and_stream_false() {
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

        let requests = server.received_requests().await.unwrap();
        let sent: serde_json::Value = serde_json::from_slice(&requests[0].body).unwrap();
        assert_eq!(sent["format"], "json");
        assert_eq!(sent["stream"], false);
        assert_eq!(sent["model"], "llama3.2");
        assert_eq!(sent["prompt"], "the prompt");
    }

    #[tokio::test]
    async fn is_available_is_true_on_200() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/tags"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .mount(&server)
            .await;
        assert!(OllamaClient::new(server.uri()).is_available().await);
    }

    /// Node checks `res.ok`, so a reachable-but-broken Ollama counts as
    /// unavailable — not just a refused connection.
    #[tokio::test]
    async fn is_available_is_false_on_500() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/tags"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;
        assert!(!OllamaClient::new(server.uri()).is_available().await);
    }

    #[tokio::test]
    async fn is_available_is_false_when_nothing_is_listening() {
        // Port 1 is reserved and never has a listener.
        assert!(!OllamaClient::new("http://127.0.0.1:1").is_available().await);
    }
}
```

- [ ] **Step 5: Run the tests to verify they fail**

```bash
cargo test -p fubbik-ai
```

Expected: FAIL — `client.rs` has no implementation yet, so this is a compile error naming `OllamaClient`.

- [ ] **Step 6: Write the implementation**

Put this **above** the `#[cfg(test)]` block in `crates/fubbik-ai/src/client.rs`:

```rust
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
            http: reqwest::Client::new(),
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

    pub async fn embed(&self, text: &str) -> Result<Vec<f32>, AiError> {
        let res = self
            .http
            .post(format!("{}/api/embeddings", self.base_url))
            .json(&serde_json::json!({ "model": EMBED_MODEL, "prompt": text }))
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
```

- [ ] **Step 7: Run the tests to verify they pass**

```bash
cargo test -p fubbik-ai
```

Expected: PASS, 12 tests.

- [ ] **Step 8: Prove the prefix tests discriminate**

Temporarily change `embed_query` to `self.embed(query).await` and re-run. Expected: `embed_query_sends_the_search_query_prefix` FAILS. Revert. Do the same for `embed_document`'s blank line — change `summary.unwrap_or("")` so the newline collapses, confirm `embed_document_keeps_the_blank_line_when_summary_is_none` fails, revert.

Record both mutation results in your report. A prefix test that passes with the prefix removed is worthless, and this is the exact vacuity class that cost Phase 4a a fix round.

- [ ] **Step 9: Lint, format, commit**

```bash
cargo fmt -p fubbik-ai
cargo clippy -p fubbik-ai --all-targets -- -D warnings
git add crates/fubbik-ai
git commit -m "feat(ai): add fubbik-ai crate with the Ollama transport

Ports packages/api/src/ollama/client.ts. The base URL is a field rather
than a call-time OLLAMA_URL lookup so tests can point the client at a
wiremock server — the reason the one existing Ollama test in this repo
can only assert the degraded path.

Prompt text stays out of this crate: prompts are domain knowledge and
belong beside the domain that owns them."
```

---

## Task 2: Wire the client onto `AppState`

This is the wide mechanical change, deliberately isolated from behaviour so a failure here is not tangled with anything else.

**Files:**
- Modify: `crates/fubbik-api/Cargo.toml` (add `fubbik-ai`; add `wiremock` to dev-dependencies)
- Modify: `crates/fubbik-api/src/lib.rs:44-51` (`AppState`)
- Modify: `crates/fubbik/src/main.rs:159-163`
- Modify: `crates/fubbik-api/src/vocabulary/{suggest,service,routes}.rs`
- Modify: `crates/fubbik-api/tests/vocabulary.rs`
- Modify: all other `crates/fubbik-api/tests/*.rs` that construct `AppState` (34 more files, each with a `fn state(pool)` helper)

**Interfaces:**
- Consumes: `fubbik_ai::OllamaClient` (Task 1)
- Produces: `AppState { pool, implicit_dev_session, better_auth_secret, ai: OllamaClient }`, and `vocabulary::suggest::suggest_vocabulary(client: &OllamaClient, chunks: &[(String, String)]) -> Vec<SuggestedEntry>` — note the dropped `ollama_url` parameter and the new leading client argument.

- [ ] **Step 1: Add the dependencies**

In `crates/fubbik-api/Cargo.toml`, add under `[dependencies]`:

```toml
fubbik-ai = { path = "../fubbik-ai" }
```

and under `[dev-dependencies]`:

```toml
wiremock = "0.6"
```

- [ ] **Step 2: Add the field**

In `crates/fubbik-api/src/lib.rs`, extend `AppState`:

```rust
    /// The Ollama transport, constructed once at startup. Carried here
    /// rather than resolved from `OLLAMA_URL` per call so that each test
    /// can inject its own `wiremock` base URL — see
    /// `crates/fubbik-ai/src/client.rs`'s module doc.
    pub ai: fubbik_ai::OllamaClient,
```

- [ ] **Step 3: Build it in main**

In `crates/fubbik/src/main.rs`, change the `AppState` literal at line 159 to:

```rust
            let state = fubbik_api::AppState {
                pool,
                implicit_dev_session,
                better_auth_secret,
                ai: fubbik_ai::OllamaClient::from_env(),
            };
```

and add `fubbik-ai = { path = "../fubbik-ai" }` to `crates/fubbik/Cargo.toml`'s `[dependencies]`.

- [ ] **Step 4: Refactor `vocabulary::suggest` onto the client**

In `crates/fubbik-api/src/vocabulary/suggest.rs`, replace the signature and the whole HTTP block. The prompt, the 8000-character truncation loop, the greedy `[`…`]` extraction and the entry validation all stay byte-for-byte as they are. Only the transport changes:

```rust
/// Infallible, matching Node's `Effect.Effect<SuggestedEntry[], never>`:
/// every failure mode degrades to an empty `Vec`.
///
/// The `ollama_url` override parameter this function used to take is gone —
/// the client now arrives from `AppState`, which supersedes it and gives
/// tests a better injection point (a real mock server rather than a URL
/// string threaded through the service layer).
pub async fn suggest_vocabulary(
    client: &fubbik_ai::OllamaClient,
    chunks: &[(String, String)],
) -> Vec<SuggestedEntry> {
    try_suggest(client, chunks).await.unwrap_or_default()
}

async fn try_suggest(
    client: &fubbik_ai::OllamaClient,
    chunks: &[(String, String)],
) -> Option<Vec<SuggestedEntry>> {
    // ... truncation loop unchanged, producing `prompt` ...

    // Node does NOT pass `format: "json"` here — it asks for a bare
    // completion and then greedily extracts the first `[` to the last `]`,
    // because llama3.2 reliably wraps the array in prose. `generate_json`
    // would reject that prose, so this path keeps the raw-string contract
    // and does its own extraction below.
    let response_text: String = client.generate_raw(&prompt, "llama3.2").await.ok()?;

    // ... extraction and validation unchanged ...
}
```

This needs one more client method. Add to `crates/fubbik-ai/src/client.rs`, and make `generate_json` call it so there is one request path:

```rust
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
            .send()
            .await
            .map_err(|e| AiError::Transport(e.to_string()))?;

        if !res.status().is_success() {
            return Err(AiError::Status(res.status().as_u16()));
        }

        let body: GenerateResponse = res.json().await.map_err(|_| AiError::Decode)?;
        Ok(body.response)
    }
```

Keep `generate_json` as written in Task 1 — it sends `format: "json"`, which `generate_raw` deliberately does not, so they are not duplicates and must not be collapsed.

Then thread the client through `vocabulary::service::suggest_from_chunks` (add a `client: &fubbik_ai::OllamaClient` parameter) and `vocabulary::routes::suggest_vocabulary` (pass `&state.ai`).

- [ ] **Step 5: Update every `AppState` construction site**

```bash
grep -rln "AppState {" crates --include='*.rs' | grep -v target
```

Expected: 42 files. In each test helper, add:

```rust
        ai: fubbik_ai::OllamaClient::new("http://127.0.0.1:1"),
```

Port 1 is reserved and never has a listener, so the default for a test that does not care about AI is a client that is definitively unavailable — the same condition those tests already run under today, now explicit rather than incidental.

- [ ] **Step 6: Replace the vocabulary suggest test with a wiremock one**

In `crates/fubbik-api/tests/vocabulary.rs`, the existing assertion at line ~372 relies on no Ollama running. Replace it with a test that proves the success path, which has never once executed:

```rust
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn suggest_returns_entries_from_the_model(pool: sqlx::PgPool) {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/api/generate"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "response": "Here you go: [{\"word\":\"user\",\"category\":\"actor\",\"expects\":[\"action\"]},{\"word\":\"x\",\"category\":\"bogus\"}]"
            })),
        )
        .mount(&server)
        .await;

    let mut st = state(pool.clone());
    st.ai = fubbik_ai::OllamaClient::new(server.uri());
    let app = fubbik_api::router(st);
    let cookie = signup(app.clone(), "a@b.test", "A").await;

    // ... create a space and a chunk in it, then POST /api/vocabulary/suggest ...

    let body = json_body(res).await;
    // The prose wrapper is stripped, the valid entry survives, and the
    // entry with an unknown category is dropped by the validation loop.
    assert_eq!(body.as_array().unwrap().len(), 1);
    assert_eq!(body[0]["word"], "user");
    assert_eq!(body[0]["category"], "actor");
    assert_eq!(body[0]["expects"][0], "action");
}
```

Keep a second test asserting the degraded path (client pointed at `http://127.0.0.1:1`, expect `200` and `[]`), so both branches are covered.

- [ ] **Step 7: Build and test**

```bash
export DATABASE_URL="postgres://postgres:password@localhost:5434/fubbik_rs"
cargo test --workspace 2>&1 | tail -20
```

Expected: baseline + the new `fubbik-ai` tests + one net new vocabulary test. No failures.

- [ ] **Step 8: Lint, format, commit**

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
git add crates/fubbik-ai crates/fubbik-api crates/fubbik/Cargo.toml crates/fubbik/src/main.rs
git commit -m "refactor(api): carry the Ollama client on AppState

Adds fubbik-ai as a dependency and threads OllamaClient through
AppState so tests can inject a wiremock base URL. vocabulary::suggest
moves onto the client and loses its ollama_url override parameter,
which the injected client supersedes.

Its test now asserts the success path — parsing entries out of a prose-
wrapped model response and dropping an invalid category — which no test
in this repository had ever executed, because the only way to reach that
code was to have a real Ollama running."
```

---

## Task 3: The enrichment write path

**Files:**
- Modify: `crates/fubbik-db/src/repo/chunk.rs`
- Test: `crates/fubbik-db/tests/chunk.rs`

**Interfaces:**
- Consumes: `fubbik_db::embedding::EmbeddingVec`
- Produces: `fubbik_db::repo::chunk::update_chunk_enrichment(pool: &PgPool, chunk_id: &str, params: EnrichmentPatch) -> AppResult<Option<Chunk>>` and `pub struct EnrichmentPatch { pub summary: Option<String>, pub aliases: Option<Vec<String>>, pub not_about: Option<Vec<String>>, pub embedding: Option<Vec<f32>> }`

- [ ] **Step 1: Write the failing tests**

Append to `crates/fubbik-db/tests/chunk.rs`:

```rust
/// The vector write path. `fubbik_db::embedding` decodes the column via
/// `embedding::text` rather than adding the `pgvector` crate; writes use the
/// mirror-image `$n::text::vector` cast. This asserts the round trip, which
/// is the only thing that proves the two halves agree on the format.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn update_chunk_enrichment_round_trips_a_vector(pool: sqlx::PgPool) {
    let user = seed_user(&pool).await;
    let chunk = seed_chunk(&pool, &user, "T", "C").await;

    let vector: Vec<f32> = (0..768).map(|i| i as f32 / 1000.0).collect();
    let updated = fubbik_db::repo::chunk::update_chunk_enrichment(
        &pool,
        &chunk.id,
        fubbik_db::repo::chunk::EnrichmentPatch {
            summary: Some("a summary".into()),
            aliases: Some(vec!["alpha".into(), "beta".into()]),
            not_about: Some(vec!["gamma".into()]),
            embedding: Some(vector.clone()),
        },
    )
    .await
    .unwrap()
    .expect("chunk exists");

    assert_eq!(updated.summary.as_deref(), Some("a summary"));
    assert_eq!(updated.aliases.0, vec!["alpha".to_string(), "beta".to_string()]);
    assert_eq!(updated.not_about.0, vec!["gamma".to_string()]);
    assert_eq!(updated.embedding.expect("written").0, vector);
    assert!(
        updated.embedding_updated_at.is_some(),
        "writing an embedding must stamp embedding_updated_at"
    );
}

/// Node spreads each field conditionally (`chunk.ts:379-383`), so `None`
/// means "leave alone", not "set to null". A patch that carries only a
/// summary must not blank an existing embedding.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn update_chunk_enrichment_leaves_absent_fields_untouched(pool: sqlx::PgPool) {
    let user = seed_user(&pool).await;
    let chunk = seed_chunk(&pool, &user, "T", "C").await;

    let vector: Vec<f32> = vec![0.5; 768];
    fubbik_db::repo::chunk::update_chunk_enrichment(
        &pool,
        &chunk.id,
        fubbik_db::repo::chunk::EnrichmentPatch {
            summary: None,
            aliases: None,
            not_about: None,
            embedding: Some(vector.clone()),
        },
    )
    .await
    .unwrap();

    let after = fubbik_db::repo::chunk::update_chunk_enrichment(
        &pool,
        &chunk.id,
        fubbik_db::repo::chunk::EnrichmentPatch {
            summary: Some("only the summary".into()),
            aliases: None,
            not_about: None,
            embedding: None,
        },
    )
    .await
    .unwrap()
    .expect("chunk exists");

    assert_eq!(after.summary.as_deref(), Some("only the summary"));
    assert_eq!(
        after.embedding.expect("must survive a summary-only patch").0,
        vector,
        "a None embedding means 'leave alone', not 'set to null'"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn update_chunk_enrichment_returns_none_for_a_missing_chunk(pool: sqlx::PgPool) {
    let got = fubbik_db::repo::chunk::update_chunk_enrichment(
        &pool,
        "does-not-exist",
        fubbik_db::repo::chunk::EnrichmentPatch {
            summary: Some("x".into()),
            aliases: None,
            not_about: None,
            embedding: None,
        },
    )
    .await
    .unwrap();
    assert!(got.is_none());
}
```

Use whatever `seed_user` / `seed_chunk` helpers already exist in that test file; if their names differ, use the existing ones rather than adding new ones.

- [ ] **Step 2: Run to verify failure**

```bash
export DATABASE_URL="postgres://postgres:password@localhost:5434/fubbik_rs"
cargo test -p fubbik-db --test chunk update_chunk_enrichment
```

Expected: FAIL — compile error, `update_chunk_enrichment` not found.

- [ ] **Step 3: Implement**

Add to `crates/fubbik-db/src/repo/chunk.rs`:

```rust
/// Sparse patch for the AI-written columns. `None` means "leave the column
/// alone" — mirroring Node's conditional spread at
/// `packages/db/src/repository/chunk.ts:379-383`, where an absent key is
/// simply not part of the `SET`.
#[derive(Debug, Default, Clone)]
pub struct EnrichmentPatch {
    pub summary: Option<String>,
    pub aliases: Option<Vec<String>>,
    pub not_about: Option<Vec<String>>,
    pub embedding: Option<Vec<f32>>,
}

/// Writes the enrichment columns and returns the updated row.
///
/// `COALESCE` gives the "absent means unchanged" semantics without building
/// the SQL dynamically, which would defeat `query_as!`'s compile-time
/// checking. `embedding_updated_at` is stamped only when an embedding is
/// actually supplied — Node ties the two together in the same conditional
/// spread, so a summary-only patch must not move the timestamp.
pub async fn update_chunk_enrichment(
    pool: &PgPool,
    chunk_id: &str,
    params: EnrichmentPatch,
) -> AppResult<Option<Chunk>> {
    // pgvector has no text input parser reachable through sqlx's inferred
    // parameter types, so the vector goes over the wire as text and is cast
    // in SQL — the exact mirror of how `embedding::text` reads it back.
    let embedding_text = params.embedding.as_ref().map(|v| {
        let joined = v
            .iter()
            .map(|f| f.to_string())
            .collect::<Vec<_>>()
            .join(",");
        format!("[{joined}]")
    });

    let aliases = params.aliases.map(serde_json::Value::from);
    let not_about = params.not_about.map(serde_json::Value::from);

    let row = sqlx::query_as!(
        Chunk,
        r#"
        UPDATE chunk SET
            summary    = COALESCE($2, summary),
            aliases    = COALESCE($3, aliases),
            not_about  = COALESCE($4, not_about),
            embedding  = COALESCE($5::text::vector, embedding),
            embedding_updated_at = CASE
                WHEN $5::text IS NULL THEN embedding_updated_at
                ELSE now()
            END
        WHERE id = $1
        RETURNING
            id, title, content, type AS chunk_type, user_id, summary,
            aliases AS "aliases: Json<Vec<String>>",
            not_about AS "not_about: Json<Vec<String>>",
            scope AS "scope: Json<serde_json::Value>",
            rationale,
            alternatives AS "alternatives: Json<Vec<String>>",
            consequences,
            embedding::text AS "embedding: EmbeddingVec",
            embedding_updated_at AS "embedding_updated_at: UtcTimestamp",
            origin, review_status, reviewed_by,
            reviewed_at AS "reviewed_at: UtcTimestamp",
            created_at AS "created_at: UtcTimestamp",
            updated_at AS "updated_at: UtcTimestamp",
            archived_at AS "archived_at: UtcTimestamp",
            document_id, document_order
        "#,
        chunk_id,
        params.summary,
        aliases,
        not_about,
        embedding_text,
    )
    .fetch_optional(pool)
    .await?;

    Ok(row)
}
```

If the `RETURNING` column list drifts from `Chunk`'s fields, copy the list verbatim from the existing `SELECT` at `crates/fubbik-db/src/repo/chunk.rs:164` — that is the authoritative projection and it already handles every type override.

- [ ] **Step 4: Run to verify pass**

```bash
cargo test -p fubbik-db --test chunk update_chunk_enrichment
```

Expected: PASS, 3 tests.

- [ ] **Step 5: Prove the "leave alone" test discriminates**

Change `embedding = COALESCE($5::text::vector, embedding)` to `embedding = $5::text::vector` and re-run. Expected: `update_chunk_enrichment_leaves_absent_fields_untouched` FAILS. Revert.

- [ ] **Step 6: Refresh the query cache and commit**

```bash
export DATABASE_URL="postgres://postgres:password@localhost:5434/fubbik_rs"
cargo sqlx prepare --workspace -- --tests
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
git add crates/fubbik-db .sqlx
git commit -m "feat(db): add the enrichment write path

update_chunk_enrichment writes summary, aliases, not_about and the
embedding, stamping embedding_updated_at only when a vector is actually
supplied. COALESCE reproduces Node's conditional-spread semantics — an
absent field means unchanged, not null — without dynamic SQL, which
would defeat query_as!'s compile-time checking.

Vectors go over the wire as text with a \$n::text::vector cast, the
mirror of the embedding::text read in fubbik_db::embedding, so no
pgvector crate dependency is introduced."
```

---

## Task 4: Semantic search and neighbour repositories

**Files:**
- Create: `crates/fubbik-db/src/repo/semantic.rs`
- Modify: `crates/fubbik-db/src/repo/mod.rs`
- Test: `crates/fubbik-db/tests/semantic.rs` (new)

**Interfaces:**
- Consumes: Task 3's vector-as-text convention
- Produces:
  - `pub struct SemanticHit { pub id: String, pub title: String, pub content: String, pub summary: Option<String>, pub chunk_type: String, pub aliases: Json<Vec<String>>, pub scope: Json<serde_json::Value>, pub similarity: f64 }`
  - `pub struct NeighborRow { pub id: String, pub title: String, pub summary: Option<String>, pub chunk_type: String, pub distance: f64 }`
  - `semantic_search(pool: &PgPool, embedding: &[f32], user_id: Option<&str>, exclude: &[String], scope: Option<&serde_json::Value>, limit: i64) -> AppResult<Vec<SemanticHit>>`
  - `find_neighbors_by_chunk_id(pool: &PgPool, chunk_id: &str, user_id: &str, k: i64) -> AppResult<Vec<NeighborRow>>`

- [ ] **Step 1: Write the failing tests**

Create `crates/fubbik-db/tests/semantic.rs`. Seed vectors directly with SQL so the tests do not depend on any model:

```rust
//! Vector-ranking tests. These run in CI: `.github/workflows/rust.yml:17`
//! uses `pgvector/pgvector:pg18`, so the `vector` extension is present even
//! though Ollama never is.

/// A 768-dimension vector that is all zeros except one hot index. Cosine
/// distance between two such vectors is 0 when the indices match and 1 when
/// they differ, which makes expected orderings exact rather than
/// approximate.
fn one_hot(index: usize) -> String {
    let mut parts = vec!["0"; 768];
    parts[index] = "1";
    format!("[{}]", parts.join(","))
}

async fn seed_chunk_with_vector(
    pool: &sqlx::PgPool,
    user_id: &str,
    id: &str,
    title: &str,
    hot: usize,
) {
    sqlx::query(
        "INSERT INTO chunk (id, title, content, type, user_id, embedding)
         VALUES ($1, $2, 'content', 'note', $3, $4::text::vector)",
    )
    .bind(id)
    .bind(title)
    .bind(user_id)
    .bind(one_hot(hot))
    .execute(pool)
    .await
    .unwrap();
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn semantic_search_orders_by_similarity(pool: sqlx::PgPool) {
    let user = seed_user(&pool).await;
    seed_chunk_with_vector(&pool, &user, "far", "Far", 5).await;
    seed_chunk_with_vector(&pool, &user, "near", "Near", 0).await;

    let mut query = vec![0.0f32; 768];
    query[0] = 1.0;

    let hits = fubbik_db::repo::semantic::semantic_search(
        &pool, &query, Some(&user), &[], None, 10,
    )
    .await
    .unwrap();

    // Order, not membership: a test that only asserts both ids are present
    // passes with the ORDER BY deleted.
    assert_eq!(
        hits.iter().map(|h| h.id.as_str()).collect::<Vec<_>>(),
        vec!["near", "far"]
    );
    assert!(hits[0].similarity > 0.99, "got {}", hits[0].similarity);
    assert!(hits[1].similarity < 0.01, "got {}", hits[1].similarity);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn semantic_search_skips_chunks_without_an_embedding(pool: sqlx::PgPool) {
    let user = seed_user(&pool).await;
    seed_chunk_with_vector(&pool, &user, "has", "Has", 0).await;
    sqlx::query(
        "INSERT INTO chunk (id, title, content, type, user_id)
         VALUES ('none', 'None', 'c', 'note', $1)",
    )
    .bind(&user)
    .execute(&pool)
    .await
    .unwrap();

    let mut query = vec![0.0f32; 768];
    query[0] = 1.0;
    let hits =
        fubbik_db::repo::semantic::semantic_search(&pool, &query, Some(&user), &[], None, 10)
            .await
            .unwrap();

    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].id, "has");
}

/// `exclude` filters on `not_about @>`, dropping a chunk that would
/// otherwise rank *first* — so deleting the filter changes the result.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn semantic_search_excludes_terms_in_not_about(pool: sqlx::PgPool) {
    let user = seed_user(&pool).await;
    seed_chunk_with_vector(&pool, &user, "near", "Near", 0).await;
    seed_chunk_with_vector(&pool, &user, "far", "Far", 5).await;
    sqlx::query("UPDATE chunk SET not_about = '[\"billing\"]'::jsonb WHERE id = 'near'")
        .execute(&pool)
        .await
        .unwrap();

    let mut query = vec![0.0f32; 768];
    query[0] = 1.0;
    let hits = fubbik_db::repo::semantic::semantic_search(
        &pool,
        &query,
        Some(&user),
        &["billing".to_string()],
        None,
        10,
    )
    .await
    .unwrap();

    assert_eq!(hits.iter().map(|h| h.id.as_str()).collect::<Vec<_>>(), vec!["far"]);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn semantic_search_filters_by_scope(pool: sqlx::PgPool) {
    let user = seed_user(&pool).await;
    seed_chunk_with_vector(&pool, &user, "near", "Near", 0).await;
    seed_chunk_with_vector(&pool, &user, "far", "Far", 5).await;
    sqlx::query("UPDATE chunk SET scope = '{\"env\":\"prod\"}'::jsonb WHERE id = 'far'")
        .execute(&pool)
        .await
        .unwrap();

    let mut query = vec![0.0f32; 768];
    query[0] = 1.0;
    let hits = fubbik_db::repo::semantic::semantic_search(
        &pool,
        &query,
        Some(&user),
        &[],
        Some(&serde_json::json!({ "env": "prod" })),
        10,
    )
    .await
    .unwrap();

    assert_eq!(hits.iter().map(|h| h.id.as_str()).collect::<Vec<_>>(), vec!["far"]);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn semantic_search_is_scoped_to_the_user(pool: sqlx::PgPool) {
    let a = seed_user(&pool).await;
    let b = seed_user(&pool).await;
    seed_chunk_with_vector(&pool, &a, "mine", "Mine", 0).await;
    seed_chunk_with_vector(&pool, &b, "theirs", "Theirs", 0).await;

    let mut query = vec![0.0f32; 768];
    query[0] = 1.0;
    let hits = fubbik_db::repo::semantic::semantic_search(&pool, &query, Some(&a), &[], None, 10)
        .await
        .unwrap();

    assert_eq!(hits.iter().map(|h| h.id.as_str()).collect::<Vec<_>>(), vec!["mine"]);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn find_neighbors_excludes_the_source_and_orders_by_distance(pool: sqlx::PgPool) {
    let user = seed_user(&pool).await;
    seed_chunk_with_vector(&pool, &user, "src", "Source", 0).await;
    seed_chunk_with_vector(&pool, &user, "near", "Near", 0).await;
    seed_chunk_with_vector(&pool, &user, "far", "Far", 7).await;

    let rows = fubbik_db::repo::semantic::find_neighbors_by_chunk_id(&pool, "src", &user, 10)
        .await
        .unwrap();

    assert_eq!(
        rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
        vec!["near", "far"],
        "the source chunk must not be its own neighbour"
    );
    assert!(rows[0].distance < rows[1].distance);
}

/// Node's query carries `AND c.archived_at IS NULL`
/// (`packages/db/src/repository/semantic.ts:41`). Semantic search itself
/// does not — the asymmetry is Node's and is preserved.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn find_neighbors_skips_archived_chunks(pool: sqlx::PgPool) {
    let user = seed_user(&pool).await;
    seed_chunk_with_vector(&pool, &user, "src", "Source", 0).await;
    seed_chunk_with_vector(&pool, &user, "gone", "Gone", 0).await;
    sqlx::query("UPDATE chunk SET archived_at = now() WHERE id = 'gone'")
        .execute(&pool)
        .await
        .unwrap();

    let rows = fubbik_db::repo::semantic::find_neighbors_by_chunk_id(&pool, "src", &user, 10)
        .await
        .unwrap();
    assert!(rows.is_empty());
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn find_neighbors_is_empty_when_the_source_has_no_embedding(pool: sqlx::PgPool) {
    let user = seed_user(&pool).await;
    sqlx::query(
        "INSERT INTO chunk (id, title, content, type, user_id)
         VALUES ('src', 'Source', 'c', 'note', $1)",
    )
    .bind(&user)
    .execute(&pool)
    .await
    .unwrap();
    seed_chunk_with_vector(&pool, &user, "other", "Other", 0).await;

    let rows = fubbik_db::repo::semantic::find_neighbors_by_chunk_id(&pool, "src", &user, 10)
        .await
        .unwrap();
    assert!(rows.is_empty(), "the CTE yields no source row, so the join yields nothing");
}
```

Reuse the `seed_user` helper from an existing `crates/fubbik-db/tests/*.rs` file; if it is not shared, copy its body rather than inventing a different user shape.

- [ ] **Step 2: Run to verify failure**

```bash
export DATABASE_URL="postgres://postgres:password@localhost:5434/fubbik_rs"
cargo test -p fubbik-db --test semantic
```

Expected: FAIL — module `semantic` does not exist.

- [ ] **Step 3: Implement**

Create `crates/fubbik-db/src/repo/semantic.rs`:

```rust
//! Vector reads behind semantic search and `{id}/neighbors`.
//!
//! Ports `packages/db/src/repository/semantic.ts`. Node builds these with
//! Drizzle's dynamic `and(...conditions)`, which cannot be reproduced under
//! `query_as!` without giving up compile-time checking. Each optional
//! filter is instead expressed as a `$n IS NULL OR <predicate>` pair, which
//! the planner short-circuits and which keeps the macro's type inference.
//!
//! `exclude` is passed as a `text[]` rather than N separate parameters
//! because its length is not known at compile time; the predicate is
//! logically Node's per-term loop (`semantic.ts:70-74`) collapsed into one
//! `NOT EXISTS`.
use fubbik_core::error::AppResult;
use sqlx::PgPool;
use sqlx::types::Json;

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SemanticHit {
    pub id: String,
    pub title: String,
    pub content: String,
    pub summary: Option<String>,
    #[serde(rename = "type")]
    pub chunk_type: String,
    #[schema(value_type = Vec<String>)]
    pub aliases: Json<Vec<String>>,
    #[schema(value_type = std::collections::HashMap<String, String>)]
    pub scope: Json<serde_json::Value>,
    pub similarity: f64,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NeighborRow {
    pub id: String,
    pub title: String,
    pub summary: Option<String>,
    #[serde(rename = "type")]
    pub chunk_type: String,
    pub distance: f64,
}

fn to_pgvector_text(embedding: &[f32]) -> String {
    let joined = embedding
        .iter()
        .map(|f| f.to_string())
        .collect::<Vec<_>>()
        .join(",");
    format!("[{joined}]")
}

pub async fn semantic_search(
    pool: &PgPool,
    embedding: &[f32],
    user_id: Option<&str>,
    exclude: &[String],
    scope: Option<&serde_json::Value>,
    limit: i64,
) -> AppResult<Vec<SemanticHit>> {
    let vector = to_pgvector_text(embedding);
    let exclude: Vec<String> = exclude.to_vec();

    let rows = sqlx::query_as!(
        SemanticHit,
        r#"
        SELECT
            c.id, c.title, c.content, c.summary,
            c.type AS chunk_type,
            c.aliases AS "aliases: Json<Vec<String>>",
            c.scope AS "scope: Json<serde_json::Value>",
            (1 - (c.embedding <=> $1::text::vector))::float8 AS "similarity!"
        FROM chunk c
        WHERE c.embedding IS NOT NULL
          AND ($2::text IS NULL OR c.user_id = $2)
          AND NOT EXISTS (
                SELECT 1 FROM unnest($3::text[]) AS term
                WHERE c.not_about @> to_jsonb(ARRAY[term])
              )
          AND ($4::jsonb IS NULL OR c.scope @> $4)
        ORDER BY c.embedding <=> $1::text::vector
        LIMIT $5
        "#,
        vector,
        user_id,
        &exclude,
        scope,
        limit,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

/// Node's `findNeighborsByChunkId` (`semantic.ts:24-61`). The source row is
/// a CTE that yields nothing when the chunk has no embedding, so the
/// `FROM chunk c, source` join produces an empty result — that is why the
/// no-embedding case needs no explicit branch here.
pub async fn find_neighbors_by_chunk_id(
    pool: &PgPool,
    chunk_id: &str,
    user_id: &str,
    k: i64,
) -> AppResult<Vec<NeighborRow>> {
    let rows = sqlx::query_as!(
        NeighborRow,
        r#"
        WITH source AS (
            SELECT embedding FROM chunk
            WHERE id = $1 AND user_id = $2 AND embedding IS NOT NULL
        )
        SELECT
            c.id, c.title, c.summary,
            c.type AS chunk_type,
            (c.embedding <=> (SELECT embedding FROM source))::float8 AS "distance!"
        FROM chunk c, source
        WHERE c.id <> $1
          AND c.user_id = $2
          AND c.embedding IS NOT NULL
          AND c.archived_at IS NULL
        ORDER BY c.embedding <=> (SELECT embedding FROM source)
        LIMIT $3
        "#,
        chunk_id,
        user_id,
        k,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows)
}
```

Register it in `crates/fubbik-db/src/repo/mod.rs` with `pub mod semantic;` in alphabetical position (between `saved_query` and `session`).

- [ ] **Step 4: Run to verify pass**

```bash
cargo test -p fubbik-db --test semantic
```

Expected: PASS, 8 tests.

- [ ] **Step 5: Prove the ordering test discriminates**

Delete the `ORDER BY` from `semantic_search` and re-run. Expected: `semantic_search_orders_by_similarity` FAILS (or becomes flaky — if it still passes, the two seeded chunks are being returned in insertion order by luck; make the test insert `far` first, which it already does, and confirm). Revert.

- [ ] **Step 6: Refresh cache, lint, commit**

```bash
export DATABASE_URL="postgres://postgres:password@localhost:5434/fubbik_rs"
cargo sqlx prepare --workspace -- --tests
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
git add crates/fubbik-db .sqlx
git commit -m "feat(db): add semantic search and neighbour reads

Ports repository/semantic.ts. Drizzle's dynamic and(...conditions) is
replaced by \$n IS NULL OR <predicate> pairs so query_as! keeps its
compile-time checking; the variable-length exclude list becomes a text[]
with one NOT EXISTS, which is Node's per-term loop collapsed.

Tests seed one-hot 768-dimension vectors directly in SQL, so cosine
distance is exactly 0 or 1 and expected orderings are exact. They run in
CI, which has pgvector."
```

---

## Task 5: The similarity repository

**Files:**
- Create: `crates/fubbik-db/src/repo/similarity.rs`
- Modify: `crates/fubbik-db/src/repo/mod.rs`
- Test: `crates/fubbik-db/tests/similarity.rs` (new)

**Interfaces:**
- Consumes: Task 4's `to_pgvector_text` convention (duplicate the helper — it is four lines and crossing module boundaries for it is not worth a `pub(crate)`)
- Produces: `pub struct SimilarChunk { pub id: String, pub title: String, pub chunk_type: String, pub similarity: f64 }` and `find_similar_by_embedding(pool: &PgPool, embedding: &[f32], user_id: &str, exclude_id: Option<&str>, threshold: f64, limit: i64) -> AppResult<Vec<SimilarChunk>>`

- [ ] **Step 1: Write the failing tests**

Create `crates/fubbik-db/tests/similarity.rs`. Reuse the `one_hot` / `seed_chunk_with_vector` helpers from Task 4 by copying them into this file.

```rust
/// The threshold must exclude, not merely rank. A test that checks only
/// that an above-threshold match comes back still passes with the filter
/// deleted entirely.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn find_similar_drops_matches_below_the_threshold(pool: sqlx::PgPool) {
    let user = seed_user(&pool).await;
    seed_chunk_with_vector(&pool, &user, "identical", "Identical", 0).await;
    seed_chunk_with_vector(&pool, &user, "orthogonal", "Orthogonal", 9).await;

    let mut query = vec![0.0f32; 768];
    query[0] = 1.0;

    let hits = fubbik_db::repo::similarity::find_similar_by_embedding(
        &pool, &query, &user, None, 0.75, 5,
    )
    .await
    .unwrap();

    assert_eq!(
        hits.iter().map(|h| h.id.as_str()).collect::<Vec<_>>(),
        vec!["identical"],
        "the orthogonal chunk has similarity 0 and must be filtered out"
    );
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn find_similar_honours_exclude_id(pool: sqlx::PgPool) {
    let user = seed_user(&pool).await;
    seed_chunk_with_vector(&pool, &user, "self", "Self", 0).await;
    seed_chunk_with_vector(&pool, &user, "other", "Other", 0).await;

    let mut query = vec![0.0f32; 768];
    query[0] = 1.0;

    let hits = fubbik_db::repo::similarity::find_similar_by_embedding(
        &pool, &query, &user, Some("self"), 0.75, 5,
    )
    .await
    .unwrap();

    assert_eq!(hits.iter().map(|h| h.id.as_str()).collect::<Vec<_>>(), vec!["other"]);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn find_similar_is_scoped_to_the_user(pool: sqlx::PgPool) {
    let a = seed_user(&pool).await;
    let b = seed_user(&pool).await;
    seed_chunk_with_vector(&pool, &a, "mine", "Mine", 0).await;
    seed_chunk_with_vector(&pool, &b, "theirs", "Theirs", 0).await;

    let mut query = vec![0.0f32; 768];
    query[0] = 1.0;

    let hits =
        fubbik_db::repo::similarity::find_similar_by_embedding(&pool, &query, &a, None, 0.75, 5)
            .await
            .unwrap();

    assert_eq!(hits.iter().map(|h| h.id.as_str()).collect::<Vec<_>>(), vec!["mine"]);
}

/// Node applies `LIMIT` in SQL and *then* filters by threshold in JS
/// (`similarity.ts:78-81`), so a below-threshold row can consume a limit
/// slot and shrink the result. That quirk is preserved: filtering in SQL
/// instead would return more rows than Node does for the same input.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn find_similar_lets_a_below_threshold_row_consume_a_limit_slot(pool: sqlx::PgPool) {
    let user = seed_user(&pool).await;
    seed_chunk_with_vector(&pool, &user, "a", "A", 0).await;
    seed_chunk_with_vector(&pool, &user, "b", "B", 3).await;
    seed_chunk_with_vector(&pool, &user, "c", "C", 0).await;

    let mut query = vec![0.0f32; 768];
    query[0] = 1.0;

    // Limit 3 pulls all three rows ordered by distance: a and c (distance
    // 0) then b (distance 1). b is then dropped by the threshold, leaving
    // two — not three.
    let hits = fubbik_db::repo::similarity::find_similar_by_embedding(
        &pool, &query, &user, None, 0.75, 3,
    )
    .await
    .unwrap();

    assert_eq!(hits.len(), 2);
    assert!(hits.iter().all(|h| h.id != "b"));
}
```

- [ ] **Step 2: Run to verify failure**

```bash
cargo test -p fubbik-db --test similarity
```

Expected: FAIL — module does not exist.

- [ ] **Step 3: Implement**

Create `crates/fubbik-db/src/repo/similarity.rs`:

```rust
//! Ports `packages/db/src/repository/similarity.ts`'s
//! `findSimilarByEmbedding`. `findDuplicatePairs` and
//! `findDuplicatePairsWithGraphSignal` are NOT ported here — their only
//! caller is the staleness duplicate scan, which is out of this slice.
use fubbik_core::error::AppResult;
use sqlx::PgPool;

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SimilarChunk {
    pub id: String,
    pub title: String,
    #[serde(rename = "type")]
    pub chunk_type: String,
    pub similarity: f64,
}

fn to_pgvector_text(embedding: &[f32]) -> String {
    let joined = embedding
        .iter()
        .map(|f| f.to_string())
        .collect::<Vec<_>>()
        .join(",");
    format!("[{joined}]")
}

/// The threshold is applied **after** `LIMIT`, in Rust, not in SQL.
///
/// That looks like a bug and is not: Node orders and limits in the query,
/// then filters the returned array (`similarity.ts:78-81`). A
/// below-threshold row therefore consumes a limit slot and shrinks the
/// result. Moving the filter into the `WHERE` clause would return *more*
/// rows than Node for the same input, which is a behaviour change, not a
/// fix.
pub async fn find_similar_by_embedding(
    pool: &PgPool,
    embedding: &[f32],
    user_id: &str,
    exclude_id: Option<&str>,
    threshold: f64,
    limit: i64,
) -> AppResult<Vec<SimilarChunk>> {
    let vector = to_pgvector_text(embedding);

    let rows = sqlx::query_as!(
        SimilarChunk,
        r#"
        SELECT
            c.id, c.title,
            c.type AS chunk_type,
            (1 - (c.embedding <=> $1::text::vector))::float8 AS "similarity!"
        FROM chunk c
        WHERE c.user_id = $2
          AND c.embedding IS NOT NULL
          AND ($3::text IS NULL OR c.id <> $3)
        ORDER BY c.embedding <=> $1::text::vector
        LIMIT $4
        "#,
        vector,
        user_id,
        exclude_id,
        limit,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .filter(|r| r.similarity >= threshold)
        .collect())
}
```

Register `pub mod similarity;` in `crates/fubbik-db/src/repo/mod.rs` (between `settings` and `space`).

- [ ] **Step 4: Run to verify pass**

```bash
cargo test -p fubbik-db --test similarity
```

Expected: PASS, 4 tests.

- [ ] **Step 5: Prove the threshold test discriminates**

Delete the `.filter(|r| r.similarity >= threshold)` and re-run. Expected: `find_similar_drops_matches_below_the_threshold` FAILS. Revert.

- [ ] **Step 6: Refresh cache, lint, commit**

```bash
export DATABASE_URL="postgres://postgres:password@localhost:5434/fubbik_rs"
cargo sqlx prepare --workspace -- --tests
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
git add crates/fubbik-db .sqlx
git commit -m "feat(db): add find_similar_by_embedding

Ports similarity.ts's findSimilarByEmbedding, including its quirk of
applying the threshold after LIMIT rather than in the WHERE clause: a
below-threshold row consumes a limit slot and shrinks the result, and a
test pins that so the 'fix' is not applied by accident."
```

---

## Task 6: The rate limiter

**Files:**
- Create: `crates/fubbik-api/src/middleware/mod.rs`
- Create: `crates/fubbik-api/src/middleware/rate_limit.rs`
- Modify: `crates/fubbik-api/src/lib.rs` (declare `pub mod middleware;`, add the limiter to `AppState`)

**Interfaces:**
- Consumes: nothing
- Produces:
  - `pub struct RateLimiter` (`Clone`, cheap — wraps an `Arc<Mutex<HashMap<..>>>`)
  - `impl RateLimiter { pub fn new() -> Self; pub fn check(&self, key: &str, max: u32, window: Duration) -> RateLimitDecision }`
  - `pub struct RateLimitDecision { pub allowed: bool, pub retry_after_secs: i64 }`
  - `AppState` gains `pub rate_limiter: RateLimiter`

- [ ] **Step 1: Write the failing tests**

At the bottom of `crates/fubbik-api/src/middleware/rate_limit.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn allows_up_to_the_limit_then_denies() {
        let limiter = RateLimiter::new();
        for i in 0..10 {
            assert!(limiter.check("k", 10, Duration::from_secs(60)).allowed, "call {i}");
        }
        assert!(!limiter.check("k", 10, Duration::from_secs(60)).allowed);
    }

    #[test]
    fn keys_do_not_share_a_window() {
        let limiter = RateLimiter::new();
        for _ in 0..10 {
            limiter.check("a", 10, Duration::from_secs(60));
        }
        assert!(
            limiter.check("b", 10, Duration::from_secs(60)).allowed,
            "one user exhausting their budget must not block another"
        );
    }

    #[test]
    fn the_window_resets_once_it_expires() {
        let limiter = RateLimiter::new();
        // A zero-length window is already expired on the next lookup, which
        // exercises the reset branch without a sleep.
        assert!(limiter.check("k", 1, Duration::from_secs(0)).allowed);
        assert!(limiter.check("k", 1, Duration::from_secs(0)).allowed);
    }

    #[test]
    fn a_denial_reports_seconds_until_reset() {
        let limiter = RateLimiter::new();
        limiter.check("k", 1, Duration::from_secs(60));
        let decision = limiter.check("k", 1, Duration::from_secs(60));
        assert!(!decision.allowed);
        assert!(
            decision.retry_after_secs > 0 && decision.retry_after_secs <= 60,
            "got {}",
            decision.retry_after_secs
        );
    }

    /// Node sweeps expired windows on a 5-minute timer
    /// (`middleware/rate-limit.ts:22-31`). This port evicts on lookup
    /// instead, so the map cannot grow without bound and no background task
    /// is needed.
    #[test]
    fn expired_entries_are_evicted_on_lookup() {
        let limiter = RateLimiter::new();
        limiter.check("gone", 1, Duration::from_secs(0));
        limiter.check("kept", 1, Duration::from_secs(60));
        assert_eq!(limiter.len(), 1);
        assert!(limiter.contains("kept"));
    }
}
```

- [ ] **Step 2: Run to verify failure**

```bash
cargo test -p fubbik-api rate_limit
```

Expected: FAIL — module does not exist.

- [ ] **Step 3: Implement**

`crates/fubbik-api/src/middleware/mod.rs`:

```rust
pub mod rate_limit;
```

`crates/fubbik-api/src/middleware/rate_limit.rs` (above the test module):

```rust
//! Ports `packages/api/src/middleware/rate-limit.ts`.
//!
//! Deliberately not an axum layer. Node applies this at two call sites —
//! enrich and semantic search — and nothing else asks for it; a tower layer
//! would be more machinery than either needs and would have to reach for
//! the session user id from inside the request extensions anyway.
//!
//! One divergence from Node: expired windows are evicted during `check`
//! rather than by a 5-minute `setInterval`. The map is keyed per user per
//! endpoint and every entry is reclaimed the next time any key is looked
//! up, so it stays bounded without a background task.
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy)]
pub struct RateLimitDecision {
    pub allowed: bool,
    /// Seconds until the current window resets. Node rounds up
    /// (`Math.ceil`), so a caller denied 0.4s before reset is told 1, never
    /// 0 — a 0 would invite an immediate retry that is still denied.
    pub retry_after_secs: i64,
}

struct Window {
    count: u32,
    reset_at: Instant,
}

#[derive(Clone, Default)]
pub struct RateLimiter {
    windows: Arc<Mutex<HashMap<String, Window>>>,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn check(&self, key: &str, max: u32, window: Duration) -> RateLimitDecision {
        let now = Instant::now();
        let mut windows = self.windows.lock().expect("rate limiter mutex poisoned");

        // Opportunistic eviction, standing in for Node's sweep timer.
        windows.retain(|_, w| w.reset_at > now);

        let entry = windows.entry(key.to_string()).or_insert_with(|| Window {
            count: 0,
            reset_at: now + window,
        });
        entry.count += 1;

        let allowed = entry.count <= max;
        let remaining = entry.reset_at.saturating_duration_since(now);
        RateLimitDecision {
            allowed,
            retry_after_secs: remaining.as_secs_f64().ceil() as i64,
        }
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.windows.lock().unwrap().len()
    }

    #[cfg(test)]
    fn contains(&self, key: &str) -> bool {
        self.windows.lock().unwrap().contains_key(key)
    }
}
```

Note the eviction runs *before* the entry lookup, so a key whose own window has expired is recreated fresh — that is what makes `the_window_resets_once_it_expires` pass.

Add to `crates/fubbik-api/src/lib.rs`: `pub mod middleware;`, and to `AppState`:

```rust
    /// Per-user request windows for the two endpoints Node rate-limits.
    /// Process-local, like Node's — this is not a distributed limiter and
    /// is not meant to be.
    pub rate_limiter: crate::middleware::rate_limit::RateLimiter,
```

Then add `rate_limiter: Default::default(),` to all 42 construction sites (the same list from Task 2) and to `crates/fubbik/src/main.rs`.

- [ ] **Step 4: Run to verify pass**

```bash
cargo test -p fubbik-api rate_limit
```

Expected: PASS, 5 tests.

- [ ] **Step 5: Lint, format, commit**

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
git add crates/fubbik-api crates/fubbik/src/main.rs
git commit -m "feat(api): add a per-user rate limiter

Ports middleware/rate-limit.ts as a plain struct on AppState rather than
a tower layer: Node applies it at exactly two call sites and nothing else
asks for one. Expired windows are evicted during check instead of by a
5-minute timer, which keeps the map bounded with no background task."
```

---

## Task 7: The enrich domain

**Files:**
- Create: `crates/fubbik-api/src/enrich/mod.rs`
- Create: `crates/fubbik-api/src/enrich/service.rs`
- Create: `crates/fubbik-api/src/enrich/routes.rs`
- Modify: `crates/fubbik-api/src/lib.rs` (declare the module, merge the router)
- Test: `crates/fubbik-api/tests/enrich.rs` (new)

**Interfaces:**
- Consumes: `OllamaClient` (Task 1), `RateLimiter` (Task 6), `repo::chunk::update_chunk_enrichment` + `EnrichmentPatch` (Task 3)
- Produces: `enrich::service::enrich_chunk(pool: &PgPool, ai: &OllamaClient, user_id: &str, chunk_id: &str) -> AppResult<Option<Chunk>>`, used by Task 10

- [ ] **Step 1: Write the failing tests**

Create `crates/fubbik-api/tests/enrich.rs`. Copy the `state` / `signup` / `send` / `json_body` helpers from `crates/fubbik-api/tests/graph.rs` — every integration test file in this crate carries its own copy.

```rust
/// A 768-length embedding response, since the column is `vector(768)` and a
/// wrong-length vector is rejected by Postgres, not silently accepted.
fn embedding_body() -> serde_json::Value {
    serde_json::json!({ "embedding": vec![0.01f32; 768] })
}

async fn ollama_mock() -> wiremock::MockServer {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/api/tags"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
        .mount(&server)
        .await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/api/generate"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "response": "{\"summary\":\"A summary.\",\"aliases\":[\"a1\",\"a2\"],\"notAbout\":[\"n1\"]}"
            })),
        )
        .mount(&server)
        .await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/api/embeddings"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(embedding_body()))
        .mount(&server)
        .await;
    server
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn enrich_writes_all_four_columns(pool: sqlx::PgPool) {
    let server = ollama_mock().await;
    let mut st = state(pool.clone());
    st.ai = fubbik_ai::OllamaClient::new(server.uri());
    let app = fubbik_api::router(st);
    let cookie = signup(app.clone(), "a@b.test", "A").await;

    let created = send(app.clone(), &cookie, "POST", "/api/chunks",
        serde_json::json!({ "title": "T", "content": "C", "type": "note" })).await;
    let id = json_body(created).await["id"].as_str().unwrap().to_string();

    let res = send(app.clone(), &cookie, "POST",
        &format!("/api/chunks/{id}/enrich"), serde_json::Value::Null).await;
    assert_eq!(res.status(), axum::http::StatusCode::OK);

    let row = sqlx::query!(
        r#"SELECT summary, aliases, not_about,
                  embedding::text AS embedding,
                  embedding_updated_at
           FROM chunk WHERE id = $1"#,
        id
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(row.summary.as_deref(), Some("A summary."));
    assert_eq!(row.aliases, serde_json::json!(["a1", "a2"]));
    assert_eq!(row.not_about, serde_json::json!(["n1"]));
    assert!(row.embedding.is_some(), "the embedding column must be written");
    assert!(row.embedding_updated_at.is_some());
}

/// Node returns `null` and writes nothing when Ollama is unreachable
/// (`enrich/service.ts:16`). The endpoint must still be a 200 — the CLI's
/// enrich-all counts non-null results and treats a failure as a skip, not
/// an error.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn enrich_is_a_no_op_when_ollama_is_unreachable(pool: sqlx::PgPool) {
    let app = fubbik_api::router(state(pool.clone())); // default client points at port 1
    let cookie = signup(app.clone(), "a@b.test", "A").await;

    let created = send(app.clone(), &cookie, "POST", "/api/chunks",
        serde_json::json!({ "title": "T", "content": "C", "type": "note" })).await;
    let id = json_body(created).await["id"].as_str().unwrap().to_string();

    let res = send(app.clone(), &cookie, "POST",
        &format!("/api/chunks/{id}/enrich"), serde_json::Value::Null).await;
    assert_eq!(res.status(), axum::http::StatusCode::OK);
    assert!(json_body(res).await.is_null());

    let row = sqlx::query!("SELECT summary FROM chunk WHERE id = $1", id)
        .fetch_one(&pool).await.unwrap();
    assert!(row.summary.is_none(), "nothing may be written when Ollama is down");
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn enrich_404s_for_a_missing_chunk(pool: sqlx::PgPool) {
    let server = ollama_mock().await;
    let mut st = state(pool.clone());
    st.ai = fubbik_ai::OllamaClient::new(server.uri());
    let app = fubbik_api::router(st);
    let cookie = signup(app.clone(), "a@b.test", "A").await;

    let res = send(app.clone(), &cookie, "POST",
        "/api/chunks/nope/enrich", serde_json::Value::Null).await;
    assert_eq!(res.status(), axum::http::StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn enrich_is_rate_limited_at_ten_per_minute(pool: sqlx::PgPool) {
    let server = ollama_mock().await;
    let mut st = state(pool.clone());
    st.ai = fubbik_ai::OllamaClient::new(server.uri());
    let app = fubbik_api::router(st);
    let cookie = signup(app.clone(), "a@b.test", "A").await;

    let created = send(app.clone(), &cookie, "POST", "/api/chunks",
        serde_json::json!({ "title": "T", "content": "C", "type": "note" })).await;
    let id = json_body(created).await["id"].as_str().unwrap().to_string();

    for i in 0..10 {
        let res = send(app.clone(), &cookie, "POST",
            &format!("/api/chunks/{id}/enrich"), serde_json::Value::Null).await;
        assert_eq!(res.status(), axum::http::StatusCode::OK, "call {i}");
    }
    let res = send(app.clone(), &cookie, "POST",
        &format!("/api/chunks/{id}/enrich"), serde_json::Value::Null).await;
    assert_eq!(res.status(), axum::http::StatusCode::TOO_MANY_REQUESTS);
    let body = json_body(res).await;
    assert_eq!(body["error"], "Rate limit exceeded");
    assert!(body["retryAfter"].as_i64().unwrap() > 0);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn enrich_all_enriches_every_chunk_and_counts_them(pool: sqlx::PgPool) {
    let server = ollama_mock().await;
    let mut st = state(pool.clone());
    st.ai = fubbik_ai::OllamaClient::new(server.uri());
    let app = fubbik_api::router(st);
    let cookie = signup(app.clone(), "a@b.test", "A").await;

    for i in 0..3 {
        send(app.clone(), &cookie, "POST", "/api/chunks",
            serde_json::json!({ "title": format!("T{i}"), "content": "C", "type": "note" })).await;
    }

    let res = send(app.clone(), &cookie, "POST",
        "/api/chunks/enrich-all", serde_json::Value::Null).await;
    assert_eq!(res.status(), axum::http::StatusCode::OK);
    assert_eq!(json_body(res).await["enriched"], 3);

    let count = sqlx::query_scalar!("SELECT count(*) FROM chunk WHERE summary IS NOT NULL")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(count, Some(3));
}

/// One user's chunks must not be enriched by another user's sweep.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn enrich_all_is_scoped_to_the_caller(pool: sqlx::PgPool) {
    let server = ollama_mock().await;
    let mut st = state(pool.clone());
    st.ai = fubbik_ai::OllamaClient::new(server.uri());
    let app = fubbik_api::router(st);

    let cookie_a = signup(app.clone(), "a@b.test", "A").await;
    let cookie_b = signup(app.clone(), "c@d.test", "C").await;
    send(app.clone(), &cookie_b, "POST", "/api/chunks",
        serde_json::json!({ "title": "B's", "content": "C", "type": "note" })).await;

    let res = send(app.clone(), &cookie_a, "POST",
        "/api/chunks/enrich-all", serde_json::Value::Null).await;
    assert_eq!(json_body(res).await["enriched"], 0);

    let count = sqlx::query_scalar!("SELECT count(*) FROM chunk WHERE summary IS NOT NULL")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(count, Some(0), "A's sweep must not touch B's chunks");
}
```

- [ ] **Step 2: Run to verify failure**

```bash
export DATABASE_URL="postgres://postgres:password@localhost:5434/fubbik_rs"
cargo test -p fubbik-api --test enrich
```

Expected: FAIL — no `enrich` module and no routes.

- [ ] **Step 3: Implement the service**

`crates/fubbik-api/src/enrich/mod.rs`:

```rust
pub mod routes;
pub mod service;
```

`crates/fubbik-api/src/enrich/service.rs`:

```rust
//! Ports `packages/api/src/enrich/service.ts`.
//!
//! The prompt lives here, not in `fubbik-ai`: it encodes what a fubbik
//! chunk is, which is domain knowledge the transport crate has no business
//! knowing.
use fubbik_ai::OllamaClient;
use fubbik_core::error::{AppError, AppResult};
use fubbik_db::repo::chunk::{self, Chunk, EnrichmentPatch};
use sqlx::PgPool;

/// Node's generation model default (`ollama/client.ts:34`).
const GENERATION_MODEL: &str = "llama3.2";

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct EnrichmentResult {
    summary: String,
    aliases: Vec<String>,
    not_about: Vec<String>,
}

/// Node's prompt verbatim, including the empty `Tags:` line — that line is
/// a latent bug in Node (tags are never interpolated), but changing it
/// changes every generated summary, so it is preserved and flagged rather
/// than quietly fixed.
fn prompt_for(chunk: &Chunk) -> String {
    format!(
        "Analyze this knowledge chunk and return JSON with these fields:\n\
         - \"summary\": a 1-2 sentence TL;DR of the content\n\
         - \"aliases\": an array of 3-8 alternative names, abbreviations, or search terms someone might use to find this\n\
         - \"notAbout\": an array of 2-5 terms this chunk could be confused with but is NOT about\n\n\
         Title: {}\n\
         Type: {}\n\
         Tags:\n\n\
         Content:\n{}",
        chunk.title, chunk.chunk_type, chunk.content
    )
}

/// `Ok(None)` means "Ollama was unavailable, nothing was written" — Node
/// returns `null` in exactly that case and the CLI's `enrich-all` counts
/// non-null results, so this distinction is load-bearing, not cosmetic.
///
/// A missing chunk is a `NotFound` error, which is checked only *after* the
/// availability probe, matching Node's ordering: with Ollama down, a
/// missing chunk id still returns null rather than 404.
pub async fn enrich_chunk(
    pool: &PgPool,
    ai: &OllamaClient,
    user_id: &str,
    chunk_id: &str,
) -> AppResult<Option<Chunk>> {
    if !ai.is_available().await {
        return Ok(None);
    }

    // Node calls `getChunkById(chunkId)` here **without** a user id
    // (`enrich/service.ts:20`), so in Node any authenticated user can
    // enrich any user's chunk by id. `chunk::find_by_id` is user-scoped and
    // there is no unscoped variant; scoping it is a deliberate tightening
    // that matches every other route in this crate, and it turns that
    // cross-user reach into a 404.
    let existing = chunk::find_by_id(pool, user_id, chunk_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Chunk".into()))?;

    // Node runs these concurrently via `Effect.all`. `try_join!` gives the
    // same shape: either both succeed or the first failure aborts.
    let (metadata, embedding) = tokio::try_join!(
        async {
            ai.generate_json::<EnrichmentResult>(&prompt_for(&existing), GENERATION_MODEL)
                .await
                .map_err(AppError::from)
        },
        async {
            ai.embed_document(
                &existing.title,
                existing.summary.as_deref(),
                &existing.content,
            )
            .await
            .map_err(AppError::from)
        }
    )?;

    chunk::update_chunk_enrichment(
        pool,
        chunk_id,
        EnrichmentPatch {
            summary: Some(metadata.summary),
            aliases: Some(metadata.aliases),
            not_about: Some(metadata.not_about),
            embedding: Some(embedding),
        },
    )
    .await
}
```

`chunk::find_by_id` is at `crates/fubbik-db/src/repo/chunk.rs:192` and its signature is `find_by_id(pool: &PgPool, user_id: &str, id: &str)`. There is no unscoped variant, which is why `user_id` is threaded in — see the comment above. Add a test for this: user B enriching user A's chunk id must get a **404**, not A's chunk enriched.

- [ ] **Step 4: Implement the routes**

`crates/fubbik-api/src/enrich/routes.rs`:

```rust
use std::time::Duration;

use axum::extract::{Path, State};
use axum::routing::post;
use axum::{Json, Router};

use crate::AppState;
use crate::auth::CurrentUser;
use crate::error::ApiResult;

/// Node's limits (`enrich/routes.ts:19`, `chunks/routes.ts:197`).
const ENRICH_MAX: u32 = 10;
const ENRICH_WINDOW: Duration = Duration::from_secs(60);

/// Node's `listChunks(userId, { limit: "1000", offset: "0" })` and
/// `{ concurrency: 3 }` (`enrich/routes.ts:34-45`). Both numbers are
/// matched exactly rather than improved: changing them is a product
/// decision, not a port decision.
const ENRICH_ALL_LIMIT: i64 = 1000;
const ENRICH_ALL_CONCURRENCY: usize = 3;

#[utoipa::path(post, path = "/api/chunks/{id}/enrich",
    params(("id" = String, Path,)),
    responses((status = 200), (status = 404), (status = 429)))]
pub async fn enrich_chunk(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
) -> ApiResult<axum::response::Response> {
    use axum::response::IntoResponse;

    let decision = state
        .rate_limiter
        .check(&format!("enrich:{}", user.id), ENRICH_MAX, ENRICH_WINDOW);
    if !decision.allowed {
        return Ok((
            axum::http::StatusCode::TOO_MANY_REQUESTS,
            Json(serde_json::json!({
                "error": "Rate limit exceeded",
                "retryAfter": decision.retry_after_secs,
            })),
        )
            .into_response());
    }

    let enriched = super::service::enrich_chunk(&state.pool, &state.ai, &user.id, &id).await?;
    Ok(Json(enriched).into_response())
}

#[utoipa::path(post, path = "/api/chunks/enrich-all", responses((status = 200)))]
pub async fn enrich_all(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
) -> ApiResult<Json<serde_json::Value>> {
    let ids = fubbik_db::repo::chunk::list_ids_for_user(&state.pool, &user.id, ENRICH_ALL_LIMIT)
        .await?;

    // Node's `Effect.forEach(..., { concurrency: 3 })` with a per-item
    // `catchAll` that yields null: one chunk failing must not abort the
    // sweep, and only successful, non-null results are counted.
    //
    // A `Semaphore` plus `JoinSet` rather than `futures::buffer_unordered`,
    // because `futures` is not a dependency of this workspace and tokio
    // (already present with `features = ["full"]`) covers it.
    let permits = std::sync::Arc::new(tokio::sync::Semaphore::new(ENRICH_ALL_CONCURRENCY));
    let mut tasks = tokio::task::JoinSet::new();
    for id in ids {
        let pool = state.pool.clone();
        let ai = state.ai.clone();
        let user_id = user.id.clone();
        let permits = permits.clone();
        tasks.spawn(async move {
            let _permit = permits.acquire().await.expect("semaphore never closed");
            super::service::enrich_chunk(&pool, &ai, &user_id, &id)
                .await
                .ok()
                .flatten()
                .is_some()
        });
    }

    let mut enriched = 0usize;
    while let Some(result) = tasks.join_next().await {
        // A panicking task counts as a failure, not an abort — same as
        // Node's per-item catchAll.
        if result.unwrap_or(false) {
            enriched += 1;
        }
    }

    Ok(Json(serde_json::json!({ "enriched": enriched })))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/chunks/{id}/enrich", post(enrich_chunk))
        .route("/api/chunks/enrich-all", post(enrich_all))
}
```

No new dependency is needed: `tokio` is already present with `features = ["full"]`, which includes `sync::Semaphore` and `task::JoinSet`.

`fubbik_db::repo::chunk::list_ids_for_user` does not exist and must be added. The existing `chunk::list` takes a large non-`Default` `ListParams` struct, so constructing one purely to fetch ids would be more code than the query below. Add:

```rust
pub async fn list_ids_for_user(pool: &PgPool, user_id: &str, limit: i64) -> AppResult<Vec<String>> {
    let ids = sqlx::query_scalar!(
        "SELECT id FROM chunk WHERE user_id = $1 ORDER BY created_at DESC LIMIT $2",
        user_id,
        limit
    )
    .fetch_all(pool)
    .await?;
    Ok(ids)
}
```

Register the module in `crates/fubbik-api/src/lib.rs` (`pub mod enrich;`) and merge `.merge(enrich::routes::router())` into `router()`. Also register both handlers in whatever utoipa `OpenApi` derive collects the paths — grep for an existing handler name like `list_chunks` in `lib.rs` to find it.

- [ ] **Step 5: Run to verify pass**

```bash
cargo test -p fubbik-api --test enrich
```

Expected: PASS, 6 tests.

- [ ] **Step 6: Prove the no-op test discriminates**

Change `enrich_chunk` to skip the `is_available` check and re-run. Expected: `enrich_is_a_no_op_when_ollama_is_unreachable` FAILS — with the probe removed, the call proceeds, `generate_json` errors on a refused connection, and the endpoint returns 502 instead of a 200 with `null`. Revert.

- [ ] **Step 7: Refresh cache, lint, commit**

```bash
export DATABASE_URL="postgres://postgres:password@localhost:5434/fubbik_rs"
cargo sqlx prepare --workspace -- --tests
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
git add crates/fubbik-api crates/fubbik-db .sqlx
git commit -m "feat(api): port the enrich domain

POST /api/chunks/{id}/enrich and /api/chunks/enrich-all, with Node's
10-per-minute limit on the former and its 1000-chunk cap and
concurrency of 3 on the latter, both matched exactly.

The prompt lives here rather than in fubbik-ai — it encodes what a
fubbik chunk is, which the transport crate has no business knowing. Its
empty 'Tags:' line is Node's latent bug and is preserved: changing it
would change every generated summary."
```

---

## Task 8: Semantic search endpoint

**Files:**
- Create: `crates/fubbik-api/src/chunks/ai.rs`
- Modify: `crates/fubbik-api/src/chunks/{mod,dto,routes}.rs`
- Test: `crates/fubbik-api/tests/chunks_ai.rs` (new)

**Interfaces:**
- Consumes: `repo::semantic::semantic_search` (Task 4), `OllamaClient`, `RateLimiter`
- Produces: `chunks::ai::semantic_search(pool, ai, user_id, query) -> AppResult<Vec<SemanticHit>>`

- [ ] **Step 1: Write the failing tests**

Create `crates/fubbik-api/tests/chunks_ai.rs` with the same helper copies and an `ollama_mock()` that answers `/api/embeddings` with a one-hot 768-vector. Cover:

```rust
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn semantic_search_returns_hits_ranked_by_similarity(pool: sqlx::PgPool) { /* two seeded
    chunks with one-hot vectors, mock returns the vector matching the nearer one, assert the
    returned ORDER */ }

/// `limit` is capped at 20 (`chunk-search.ts:60`). A request for 100 must
/// not return 100.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn semantic_search_caps_limit_at_twenty(pool: sqlx::PgPool) { /* seed 25 chunks with
    vectors, request limit=100, assert exactly 20 come back */ }

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn semantic_search_parses_scope_pairs_and_drops_malformed_ones(pool: sqlx::PgPool) {
    /* scope="env:prod,garbage" — the well-formed pair filters, the malformed one is
       discarded rather than erroring */ }

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn semantic_search_is_rate_limited_at_thirty_per_minute(pool: sqlx::PgPool) { /* 30 OK,
    31st is 429 with retryAfter */ }

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn semantic_search_502s_when_ollama_is_unreachable(pool: sqlx::PgPool) {
    /* Node has no availability probe on this path — generateQueryEmbedding failing
       propagates as AiError → 502. Assert 502, NOT an empty 200. */ }
```

Write each of these out in full, following the shapes in Task 7's test file. Do not leave the comment bodies as the implementation.

- [ ] **Step 2: Run to verify failure**

```bash
cargo test -p fubbik-api --test chunks_ai
```

Expected: FAIL — route not found (404s) or compile error.

- [ ] **Step 3: Implement**

`crates/fubbik-api/src/chunks/ai.rs`:

```rust
//! The three chunk endpoints that need embeddings.
//!
//! Ports the embedding-dependent half of
//! `packages/api/src/chunks/chunk-search.ts` plus
//! `packages/api/src/chunks/similarity.ts`.
use fubbik_ai::OllamaClient;
use fubbik_core::error::{AppError, AppResult};
use fubbik_db::repo::semantic::{self, SemanticHit};
use fubbik_db::repo::similarity::{self, SimilarChunk};
use sqlx::PgPool;

/// `Math.min(Number(query.limit ?? 5), 20)` (`chunk-search.ts:60`).
fn clamp_limit(limit: Option<i64>) -> i64 {
    limit.unwrap_or(5).min(20).max(1)
}

/// `"a:1,b:2"` → `{"a":"1","b":"2"}`. Node splits on `:` and keeps only
/// pairs of length exactly 2 (`chunk-search.ts:62-69`), so `"a:b:c"` and
/// `"bare"` are both discarded rather than erroring.
fn parse_scope(raw: &str) -> Option<serde_json::Value> {
    let map: serde_json::Map<String, serde_json::Value> = raw
        .split(',')
        .filter_map(|pair| {
            let parts: Vec<&str> = pair.trim().split(':').collect();
            if parts.len() == 2 {
                Some((parts[0].to_string(), serde_json::Value::from(parts[1])))
            } else {
                None
            }
        })
        .collect();
    if map.is_empty() {
        None
    } else {
        Some(serde_json::Value::Object(map))
    }
}

/// No availability probe here, unlike `enrich`. Node calls
/// `generateQueryEmbedding` directly, so an unreachable Ollama surfaces as
/// an `AiError` → **502**, not as an empty result set. Adding a probe would
/// turn a hard failure into a silent empty page.
pub async fn semantic_search(
    pool: &PgPool,
    ai: &OllamaClient,
    user_id: &str,
    q: &str,
    limit: Option<i64>,
    exclude: Option<&str>,
    scope: Option<&str>,
) -> AppResult<Vec<SemanticHit>> {
    let embedding = ai.embed_query(q).await.map_err(AppError::from)?;
    let exclude: Vec<String> = exclude
        .map(|raw| raw.split(',').map(|s| s.trim().to_string()).collect())
        .unwrap_or_default();
    let scope = scope.and_then(parse_scope);

    semantic::semantic_search(
        pool,
        &embedding,
        Some(user_id),
        &exclude,
        scope.as_ref(),
        clamp_limit(limit),
    )
    .await
}
```

Add a `#[cfg(test)] mod tests` in this file covering `clamp_limit` and `parse_scope` directly — including `parse_scope("a:b:c")` returning `None` and `parse_scope("x:1,bad")` keeping only `x`.

Add the DTO to `crates/fubbik-api/src/chunks/dto.rs`:

```rust
/// Query for `GET /api/chunks/search/semantic`. All values arrive as
/// strings from Node's schema, so `limit` is parsed rather than typed —
/// keeping the wire contract identical.
#[derive(Debug, serde::Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct SemanticSearchQuery {
    pub q: String,
    pub limit: Option<String>,
    pub exclude: Option<String>,
    pub scope: Option<String>,
}
```

And the handler in `crates/fubbik-api/src/chunks/routes.rs`, following `list_chunks`'s shape, applying the 30/60s limit keyed `semantic-search:{user_id}` and returning the same 429 body as Task 7. Register the route in that file's `router()`.

- [ ] **Step 4: Run to verify pass**

```bash
cargo test -p fubbik-api --test chunks_ai
```

Expected: PASS.

- [ ] **Step 5: Prove the limit cap discriminates**

Change `clamp_limit` to `limit.unwrap_or(5)` and re-run. Expected: `semantic_search_caps_limit_at_twenty` FAILS. Revert.

- [ ] **Step 6: Refresh cache, lint, commit**

```bash
export DATABASE_URL="postgres://postgres:password@localhost:5434/fubbik_rs"
cargo sqlx prepare --workspace -- --tests
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
git add crates/fubbik-api .sqlx
git commit -m "feat(api): port GET /api/chunks/search/semantic

Rate limited at 30/min per user, limit capped at 20, exclude and scope
parsed exactly as Node parses them — malformed scope pairs discarded
rather than rejected.

No availability probe on this path, unlike enrich: Node calls
generateQueryEmbedding directly, so an unreachable Ollama is a 502, and
probing would turn a hard failure into a silently empty result page."
```

---

## Task 9: check-similar and neighbors

**Files:**
- Modify: `crates/fubbik-api/src/chunks/{ai,dto,routes}.rs`
- Test: `crates/fubbik-api/tests/chunks_ai.rs`

**Interfaces:**
- Consumes: `repo::similarity::find_similar_by_embedding` (Task 5), `repo::semantic::find_neighbors_by_chunk_id` (Task 4), `fubbik_db::age::get_neighborhood` (already exists at `crates/fubbik-db/src/age.rs:450`)
- Produces: `NeighborsResponse { neighbors: Vec<NeighborItem>, note: Option<String> }` where `NeighborItem` adds `embedding_similarity: f64`, `graph_connected: bool`, `combined_score: f64` to `NeighborRow`'s fields

- [ ] **Step 1: Write the failing tests**

Append to `crates/fubbik-api/tests/chunks_ai.rs`:

```rust
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn check_similar_returns_only_matches_at_or_above_the_threshold(pool: sqlx::PgPool) {
    /* seed an identical-vector chunk and an orthogonal one; mock returns the query
       vector; assert exactly the identical one comes back and the orthogonal one does
       not — the 0.75 threshold this call site passes, not the repo's 0.7 default */
}

/// Node returns `[]`, not a 502, when Ollama is down here — `checkSimilar`
/// probes availability first (`similarity.ts:9`). This is the opposite of
/// semantic search, and the asymmetry is Node's.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn check_similar_returns_empty_when_ollama_is_unreachable(pool: sqlx::PgPool) { /* ... */ }

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn check_similar_caps_at_three(pool: sqlx::PgPool) { /* seed 5 identical-vector chunks,
    assert exactly 3 come back */ }

/// No Ollama call at all on this path: the source chunk's stored embedding
/// is used, not a freshly generated one. The mock asserts zero requests.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn neighbors_notes_a_missing_embedding_without_calling_ollama(pool: sqlx::PgPool) {
    /* create a chunk with no embedding; GET /api/chunks/{id}/neighbors;
       assert neighbors == [] and note == "Chunk has no embedding — run enrichment first.";
       assert server.received_requests() is empty */
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn neighbors_are_ordered_by_combined_score(pool: sqlx::PgPool) { /* three chunks with
    distinct one-hot vectors, assert descending combinedScore and that note is null */ }

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn neighbors_k_is_clamped_between_one_and_fifty(pool: sqlx::PgPool) { /* k=0 and k=999 */ }

/// The 0.15 graph bonus must be able to REORDER, not just decorate: seed a
/// slightly-worse embedding match that is graph-connected and assert it
/// overtakes a slightly-better one that is not.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn the_graph_bonus_reorders_neighbours(pool: sqlx::PgPool) {
    if !fubbik_db::age::is_available(&pool).await {
        eprintln!("AGE unavailable in this database — skipping");
        return;
    }
    /* ... */
}
```

Write all seven out in full.

- [ ] **Step 2: Run to verify failure**

```bash
cargo test -p fubbik-api --test chunks_ai
```

Expected: the seven new tests FAIL; Task 8's still pass.

- [ ] **Step 3: Implement**

Add to `crates/fubbik-api/src/chunks/ai.rs`:

```rust
/// Node's `checkSimilar` call-site constants (`similarity.ts:13-15`) —
/// threshold 0.75 and limit 3, both overriding the repository defaults of
/// 0.7 and 5.
const CHECK_SIMILAR_THRESHOLD: f64 = 0.75;
const CHECK_SIMILAR_LIMIT: i64 = 3;

/// Probes availability and returns `[]` when Ollama is down — the opposite
/// of `semantic_search`, which 502s. The asymmetry is Node's
/// (`similarity.ts:9` has the probe; `chunk-search.ts:71` does not) and is
/// preserved deliberately: this endpoint runs while a user types in the
/// create form, where a hard error would be hostile.
pub async fn check_similar(
    pool: &PgPool,
    ai: &OllamaClient,
    user_id: &str,
    title: &str,
    content: &str,
    exclude_id: Option<&str>,
) -> AppResult<Vec<SimilarChunk>> {
    if !ai.is_available().await {
        return Ok(Vec::new());
    }
    let embedding = ai
        .embed_document(title, None, content)
        .await
        .map_err(AppError::from)?;
    similarity::find_similar_by_embedding(
        pool,
        &embedding,
        user_id,
        exclude_id,
        CHECK_SIMILAR_THRESHOLD,
        CHECK_SIMILAR_LIMIT,
    )
    .await
}

/// Node's graph bonus (`semantic.ts:105`).
const GRAPH_BONUS: f64 = 0.15;
/// Node's `graphHops` default (`semantic.ts:99`).
const GRAPH_HOPS: i32 = 2;

#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NeighborItem {
    pub id: String,
    pub title: String,
    pub summary: Option<String>,
    #[serde(rename = "type")]
    pub chunk_type: String,
    pub distance: f64,
    pub embedding_similarity: f64,
    pub graph_connected: bool,
    pub combined_score: f64,
}

#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NeighborsResponse {
    pub neighbors: Vec<NeighborItem>,
    pub note: Option<String>,
}

/// Never calls Ollama: the source chunk's *stored* embedding drives this,
/// so a chunk that has never been enriched gets a note rather than a
/// freshly generated vector.
pub async fn neighbors(
    pool: &PgPool,
    user_id: &str,
    chunk_id: &str,
    k: i64,
) -> AppResult<NeighborsResponse> {
    let source = fubbik_db::repo::chunk::find_by_id(pool, user_id, chunk_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Chunk".into()))?;

    if source.embedding.is_none() {
        return Ok(NeighborsResponse {
            neighbors: Vec::new(),
            note: Some("Chunk has no embedding — run enrichment first.".to_string()),
        });
    }

    // Node over-fetches `k * 2` so the graph bonus has room to reorder
    // before the final truncation to `k`.
    let rows = semantic::find_neighbors_by_chunk_id(pool, chunk_id, user_id, k * 2).await?;

    // AGE failure degrades to "no bonus", matching Node's `Effect.catchAll`
    // — a missing graph must not fail the endpoint.
    let graph_ids: std::collections::HashSet<String> =
        fubbik_db::age::get_neighborhood(pool, chunk_id, GRAPH_HOPS)
            .await
            .unwrap_or_default()
            .into_iter()
            .collect();

    let mut scored: Vec<NeighborItem> = rows
        .into_iter()
        .map(|r| {
            let graph_connected = graph_ids.contains(&r.id);
            let embedding_similarity = 1.0 - r.distance;
            NeighborItem {
                id: r.id,
                title: r.title,
                summary: r.summary,
                chunk_type: r.chunk_type,
                distance: r.distance,
                embedding_similarity,
                graph_connected,
                combined_score: embedding_similarity
                    + if graph_connected { GRAPH_BONUS } else { 0.0 },
            }
        })
        .collect();

    scored.sort_by(|a, b| {
        b.combined_score
            .partial_cmp(&a.combined_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    scored.truncate(k as usize);

    Ok(NeighborsResponse {
        neighbors: scored,
        note: None,
    })
}
```

Add the route handlers to `crates/fubbik-api/src/chunks/routes.rs`: `POST /api/chunks/check-similar` (body `{title, content, excludeId?}`) and `GET /api/chunks/{id}/neighbors` (query `k`, clamped `1..=50` per `chunks/routes.ts:303`). Register both.

- [ ] **Step 4: Run to verify pass**

```bash
cargo test -p fubbik-api --test chunks_ai
```

Expected: PASS, all of Task 8's and Task 9's tests.

- [ ] **Step 5: Prove the graph bonus test discriminates**

Set `GRAPH_BONUS` to `0.0` and re-run. Expected: `the_graph_bonus_reorders_neighbours` FAILS. If it still passes, the two seeded neighbours are too far apart for 0.15 to flip them — tighten the seeded vectors until it does. Revert.

- [ ] **Step 6: Refresh cache, lint, commit**

```bash
export DATABASE_URL="postgres://postgres:password@localhost:5434/fubbik_rs"
cargo sqlx prepare --workspace -- --tests
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
git add crates/fubbik-api .sqlx
git commit -m "feat(api): port check-similar and {id}/neighbors

check-similar probes Ollama and degrades to [] — the opposite of semantic
search, which 502s. That asymmetry is Node's and is deliberate: this
endpoint fires while a user types in the create form.

neighbors never calls Ollama; it reads the source chunk's stored vector
and notes its absence. The 0.15 graph bonus is applied over k*2
candidates before truncating to k, so it can genuinely reorder, and a
test pins that it does."
```

---

## Task 10: Re-enrich on edit

**Files:**
- Modify: `crates/fubbik-api/src/chunks/routes.rs` (the `PATCH` handler)
- Test: `crates/fubbik-api/tests/chunks_ai.rs`

**Interfaces:**
- Consumes: `enrich::service::enrich_chunk` (Task 7)
- Produces: nothing new

- [ ] **Step 1: Write the failing tests**

```rust
/// The re-enrich is detached, so this polls with a deadline rather than
/// sleeping: a sleep long enough to be reliable is long enough to slow the
/// suite, and a short one is flaky.
async fn wait_for_summary(pool: &sqlx::PgPool, id: &str) -> Option<String> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while std::time::Instant::now() < deadline {
        let row = sqlx::query!("SELECT summary FROM chunk WHERE id = $1", id)
            .fetch_one(pool)
            .await
            .unwrap();
        if row.summary.is_some() {
            return row.summary;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    None
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn patching_the_title_triggers_a_re_enrich(pool: sqlx::PgPool) { /* PATCH title, then
    wait_for_summary returns Some("A summary.") */ }

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn patching_the_content_triggers_a_re_enrich(pool: sqlx::PgPool) { /* ... */ }

/// Node gates on `title !== undefined || content !== undefined`
/// (`chunk-mutations.ts:213`). A PATCH of any other field must not spend an
/// Ollama call.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn patching_another_field_does_not_trigger_a_re_enrich(pool: sqlx::PgPool) {
    /* PATCH rationale only; assert the mock received zero /api/generate requests
       after a short settle window, and summary stays NULL */
}

/// The spawned task must not be able to fail the request.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn a_failing_re_enrich_does_not_fail_the_patch(pool: sqlx::PgPool) {
    /* client points at port 1; PATCH title; assert 200 and the returned body is the
       updated chunk */
}
```

Write all four out in full.

- [ ] **Step 2: Run to verify failure**

Expected: the three positive tests FAIL (summary stays `NULL`); `a_failing_re_enrich_does_not_fail_the_patch` passes vacuously for now.

- [ ] **Step 3: Implement**

In `crates/fubbik-api/src/chunks/routes.rs`'s PATCH handler, capture the gate **before** `body` is moved into the service call, then spawn after it returns:

```rust
    // Node fires this from the service layer (`chunk-mutations.ts:213-217`).
    // Here it lives in the route because `chunks::service::update` takes
    // only a `&PgPool` — threading the Ollama client through every service
    // function to reach one call site would be a wider change than the
    // behaviour warrants, and the route is the only layer that already
    // holds both.
    let should_reenrich = body.title.is_some() || body.content.is_some();

    let updated = service::update(&state.pool, &user.id, &id, body).await?;

    if should_reenrich {
        let pool = state.pool.clone();
        let ai = state.ai.clone();
        let chunk_id = id.clone();
        let user_id = user.id.clone();
        tokio::spawn(async move {
            if let Err(err) = crate::enrich::service::enrich_chunk(&pool, &ai, &user_id, &chunk_id).await {
                tracing::error!("[enrich] failed to re-enrich chunk {chunk_id}: {err}");
            }
        });
    }

    Ok(Json(updated))
```

Note this deliberately reproduces Node's *full* re-enrich, which regenerates `summary`, `aliases` and `not_about` as well as the embedding — overwriting a hand-written summary. That is Node's behaviour today; changing it is a product decision, not a port decision.

- [ ] **Step 4: Run to verify pass**

```bash
cargo test -p fubbik-api --test chunks_ai
```

Expected: PASS.

- [ ] **Step 5: Prove the gate discriminates**

Change `should_reenrich` to `true` unconditionally and re-run. Expected: `patching_another_field_does_not_trigger_a_re_enrich` FAILS. Revert.

- [ ] **Step 6: Lint, format, commit**

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
git add crates/fubbik-api
git commit -m "feat(api): re-enrich on title or content edit

Closes the parity gap that motivated this slice: Rust never refreshed
embeddings, so after cutover every edited chunk would have kept a stale
vector while CLAUDE.md promised otherwise.

Fired from the route rather than the service because chunks::service
takes only a pool, and the route is the only layer already holding both
it and the Ollama client. Detached, with failures logged — a PATCH must
not fail because Ollama is down."
```

---

## Task 11: Un-stub unified search's `similar-to`

**Files:**
- Modify: `crates/fubbik-api/src/search/service.rs` (module doc, `resolve_graph_clauses`, `execute_search`)
- Modify: `crates/fubbik-api/src/search/routes.rs:43`
- Test: `crates/fubbik-api/tests/search.rs` (including the existing call at line 835)

**Interfaces:**
- Consumes: `repo::semantic::semantic_search`, `OllamaClient`
- Produces: `execute_search(pool: &PgPool, ai: &OllamaClient, user_id: &str, query: &SearchQueryBody) -> SearchResult` — note the new second parameter

- [ ] **Step 1: Write the failing test**

Append to `crates/fubbik-api/tests/search.rs`:

```rust
/// `similar-to:` has resolved to zero ids since this port began, because no
/// embedding pipeline existed (`search/service.rs:399`'s comment). This is
/// the first test that proves it resolves to something.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn similar_to_resolves_real_ids(pool: sqlx::PgPool) {
    /* seed two chunks with one-hot vectors; mock /api/embeddings to return the vector
       matching the nearer one; run a query with a similar-to clause; assert the near
       chunk is in the results and the far one is not, and that graphMeta.type is
       still the literal "semantic" */
}

/// Node wraps the whole resolution in `Effect.orElse(() => [])`
/// (`search/service.ts:117-121`), so an unreachable Ollama yields an empty
/// clause, not an error — unlike the standalone semantic endpoint.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn similar_to_degrades_to_empty_when_ollama_is_down(pool: sqlx::PgPool) { /* ... */ }
```

- [ ] **Step 2: Run to verify failure**

```bash
cargo test -p fubbik-api --test search similar_to
```

Expected: `similar_to_resolves_real_ids` FAILS (zero results).

- [ ] **Step 3: Implement**

Thread `ai: &OllamaClient` through `execute_search` → `resolve_graph_clauses`, and replace the stub at `crates/fubbik-api/src/search/service.rs:397-410`:

```rust
            "similar-to" => {
                // Node: generateQueryEmbedding → semanticSearch(limit 20) →
                // ids, the whole chain wrapped in `Effect.orElse(() => [])`
                // (`search/service.ts:117-121`). An unreachable Ollama
                // therefore yields an empty clause rather than an error —
                // deliberately unlike `GET /api/chunks/search/semantic`,
                // which surfaces the failure as a 502.
                let resolved: Vec<String> = match ai.embed_query(&clause.value).await {
                    Ok(embedding) => {
                        fubbik_db::repo::semantic::semantic_search(
                            pool, &embedding, Some(user_id), &[], None, 20,
                        )
                        .await
                        .map(|hits| hits.into_iter().map(|h| h.id).collect())
                        .unwrap_or_default()
                    }
                    Err(_) => Vec::new(),
                };
                out.ids = Some(intersect_ids(out.ids, resolved));
                // ... the existing GraphMeta / similar_to_query assignment, unchanged ...
            }
```

`resolve_graph_clauses` may not currently receive `user_id`; pass it through if not.

Then update both call sites: `crates/fubbik-api/src/search/routes.rs:43` gains `&state.ai`, and `crates/fubbik-api/tests/search.rs:835` gains a client (use `fubbik_ai::OllamaClient::new("http://127.0.0.1:1")` there unless that test exercises `similar-to`).

Finally, correct the now-false module documentation at `crates/fubbik-api/src/search/service.rs:24-25` and `:51` — both assert that no Ollama pipeline exists in this port. Leaving them would be exactly the stale-comment class caught in Phase 4a.

- [ ] **Step 4: Run to verify pass**

```bash
cargo test -p fubbik-api --test search
```

Expected: PASS, including the pre-existing search tests.

- [ ] **Step 5: Confirm no stale claims remain**

```bash
grep -rn "no Ollama\|no embedding/Ollama\|pipeline exists" crates/fubbik-api/src
```

Expected: no output.

- [ ] **Step 6: Lint, format, commit**

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
git add crates/fubbik-api
git commit -m "feat(search): resolve similar-to against real embeddings

The clause has resolved to zero ids since this port began, with a comment
saying so. It now embeds the query and runs a semantic search capped at
20, degrading to an empty clause when Ollama is unreachable — Node's
Effect.orElse, deliberately unlike the standalone semantic endpoint,
which 502s.

Removes the three module comments asserting that no Ollama pipeline
exists in this port; they are no longer true."
```

---

## Task 12: Move the web call sites onto Rust

**Files:**
- Modify: `apps/web/src/utils/api-helpers.ts:22`
- Modify: `apps/web/src/routes/knowledge-health.tsx:285`
- Modify: `apps/web/src/features/chunks/related-suggestions.tsx:25`
- Modify: `apps/web/src/features/chunks/similar-chunks-warning.tsx:23`
- Modify: `apps/web/src/features/chunks/detail/chunk-neighbors.tsx:30`
- Modify: `apps/web/src/utils/api.ts` (the hybrid-client doc block at lines ~94-96)
- Regenerate: `apps/web/src/utils/api-types.ts`

**Interfaces:**
- Consumes: every route from Tasks 7-9
- Produces: nothing

- [ ] **Step 1: Regenerate the typed client**

Find the generation script (grep `openapi-typescript` in `apps/web/package.json`) and run it against a locally running **Rust** server. Do not start the Node server.

- [ ] **Step 2: Migrate the five call sites**

Change `legacyApi` to `api` in each. Eden's `{ data, error }` destructuring differs from the generated client's shape — follow the pattern already used by a migrated call site such as `apps/web/src/features/graph/use-graph-data.ts:27`, and use `unwrapEden` only where the file still talks to `legacyApi`.

Watch for `?? null` normalisation: utoipa omits always-serialised `Option` fields from `required`, so generated types admit `undefined` where the wire never sends it. Phase 4a hit this at `graph-view.tsx:112`; `summary` and `note` are the likely spots here.

- [ ] **Step 3: Update the doc block**

In `apps/web/src/utils/api.ts`, remove `search/semantic`, `check-similar`, `{id}/neighbors` and `{id}/enrich` from the "no Rust route" list, leaving `search/federated`, `grouped`, `clusters`, `import-docs`, `{id}/suggestions` and the `ai` domain. Keep the three-state distinction the block already draws.

- [ ] **Step 4: Type-check**

```bash
pnpm run check-types
```

Expected: 7/7 successful, 0 errors.

- [ ] **Step 5: Confirm nothing is left behind**

```bash
grep -rn "legacyApi" apps/web/src | grep -E "enrich|semantic|check-similar|neighbors"
```

Expected: no output.

- [ ] **Step 6: Commit**

```bash
git add apps/web/src
git commit -m "feat(web): move the five AI call sites onto the Rust API

enrich, semantic search, check-similar and neighbors now go through the
generated client rather than Eden. Updates the hybrid-client doc block in
api.ts to match."
```

---

## Task 13: Final verification

**Files:** none

**Interfaces:**
- Consumes: every prior task
- Produces: the evidence that the exit criteria in the spec are met

- [ ] **Step 1: Full suite**

```bash
export DATABASE_URL="postgres://postgres:password@localhost:5434/fubbik_rs"
cargo test --workspace 2>&1 | tail -40
```

Expected: no failures, and a total strictly greater than Task 0's baseline.

- [ ] **Step 2: Confirm no AI test was skipped**

```bash
cargo test --workspace 2>&1 | grep -i "skipping"
```

Expected: only AGE skips, if any — never an Ollama skip. Every wiremock-backed test must have run.

- [ ] **Step 3: Lint, format, offline build**

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --check
SQLX_OFFLINE=true cargo check --workspace --all-targets
```

Expected: all three exit 0.

- [ ] **Step 4: Web type-check**

```bash
pnpm run check-types
```

Expected: 7/7 successful.

- [ ] **Step 5: Confirm the diff is additive where it should be**

```bash
BASE=$(git merge-base HEAD main)
git diff --stat "$BASE"...HEAD -- .sqlx | tail -1
git diff "$BASE"...HEAD -- crates/fubbik-db/migrations | grep -E "^\+" | grep -vE "^\+\s*--|^\+\+\+"
```

Expected: `.sqlx` shows insertions and **0 deletions**; the migrations grep is empty (this slice adds no SQL migrations — the `vector` column already exists from `0001_init.sql`).

Note: the branch point is not `main`'s tip. Phase 4a learned this the hard way — use `git merge-base` as above, never a bare `main`.

- [ ] **Step 6: Update CLAUDE.md**

Correct the Ollama section: it currently says "Embeddings auto-refresh on title/content edit (fire-and-forget, logged on failure)", which understates what happens — the whole enrichment refreshes, including summary, aliases and notAbout. Say that plainly.

```bash
git add CLAUDE.md
git commit -m "docs: correct what refreshes on chunk edit

CLAUDE.md said embeddings auto-refresh on title/content edit. The whole
enrichment does — summary, aliases and notAbout too, overwriting a
hand-written summary. That has always been Node's behaviour and is now
Rust's; the documentation just never said so."
```

- [ ] **Step 7: Report**

State the final test counts against Task 0's baseline, every mutation-test result collected along the way, and any divergence from Node that was introduced and why.

---

## Self-Review

**Spec coverage.** Every spec section maps to a task: the `fubbik-ai` crate → Task 1; injection via `AppState` → Task 2; the `vocabulary/suggest.rs` refactor → Task 2; the vector write path → Task 3; semantic search and neighbour reads → Task 4; `find_similar_by_embedding` → Task 5; rate limiting → Task 6; enrich and enrich-all → Task 7; semantic search endpoint → Task 8; check-similar and neighbors → Task 9; re-enrich on edit → Task 10; un-stubbing unified search → Task 11; the web migration → Task 12; exit criteria → Task 13.

**Two things this plan adds that the spec did not name.** `generate_raw` (Task 2) — discovered while reading `vocabulary/suggest.rs`, which deliberately does *not* use Ollama's JSON mode because llama3.2 wraps its array in prose; `generate_json` alone could not serve it. And `list_ids_for_user` (Task 7), needed by `enrich-all`, which the spec described only in prose.

**Repository names, resolved before writing.** `chunk::find_by_id(pool, user_id, id)` is the only chunk-by-id function and it is user-scoped (`crates/fubbik-db/src/repo/chunk.rs:192`); `list_ids_for_user` does not exist and Task 7 adds it. The scoping forces one behavioural divergence: Node's `enrichChunk` reads the chunk **without** a user id, so in Node any authenticated user can enrich any chunk by id. Rust scopes it, turning that into a 404. Task 7 documents this at the call site and adds a test for it.

**Divergences from Node, all deliberate and each documented at its site.** Rate-limit eviction on lookup instead of a sweep timer (Task 6); `AiError` splitting transport from decode failures (Task 1); the re-enrich firing from the route rather than the service (Task 10); enrich scoped to the calling user (Task 7). Two Node quirks are deliberately *preserved* rather than fixed: the threshold applied after `LIMIT` in `find_similar_by_embedding` (Task 5) and the empty `Tags:` line in the enrichment prompt (Task 7).
