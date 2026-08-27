//! Ollama transport. Knows Ollama's HTTP contract and nothing about fubbik.
//!
//! No prompt text lives in this crate: prompts are domain knowledge and
//! belong beside the domain that owns them (`fubbik-api`'s `enrich` and
//! `vocabulary` modules each keep their own).
pub mod client;
pub mod error;

pub use client::OllamaClient;
pub use error::AiError;
