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
