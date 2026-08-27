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
///
/// **Divergence from Node**: Node's `enrichChunk` calls `getChunkById(chunkId)`
/// with **no** user id (`enrich/service.ts:20`), so today any authenticated
/// user can trigger enrichment on any other user's chunk by id, overwriting
/// its summary/aliases/notAbout with model output. `chunk::find_by_id` here
/// is user-scoped and there is no unscoped variant, so this port closes
/// that hole by construction — a foreign chunk id now 404s instead of being
/// silently enriched. See `crates/fubbik-api/tests/enrich.rs`'s
/// `enrich_404s_and_leaves_another_users_chunk_untouched` for the pinning
/// test.
pub async fn enrich_chunk(
    pool: &PgPool,
    ai: &OllamaClient,
    user_id: &str,
    chunk_id: &str,
) -> AppResult<Option<Chunk>> {
    if !ai.is_available().await {
        return Ok(None);
    }

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
