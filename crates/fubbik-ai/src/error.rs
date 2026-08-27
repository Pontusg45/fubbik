//! `AiError` variants carry whatever detail is useful for logging —
//! including, for `Transport`, `reqwest`'s error text, which embeds the
//! request URL. None of that reaches the client: `From<AiError> for
//! AppError` below logs the full error and maps it onto a fixed
//! `"AI service error"` message, mirroring Node's `Effect.catchAll` at
//! `packages/api/src/index.ts:193-196`.
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
        // Node logs the cause and returns a fixed string
        // (`packages/api/src/index.ts:193-196`). Prompts, model output and
        // the Ollama URL can all reach `err`, so nothing derived from it
        // goes to the client.
        tracing::warn!("ai service error: {err}");
        AppError::External("AI service error".into())
    }
}
