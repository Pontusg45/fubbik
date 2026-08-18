//! Business logic for the `vocabulary` domain
//! (`packages/api/src/vocabulary/service.ts`). Every operation authorizes
//! via `verify_space_ownership` — the space the entry lives in (or will
//! live in) must belong to the caller — never via `vocabulary_entry.user_id`
//! directly; see `fubbik_db::repo::vocabulary`'s module doc for why.

use fubbik_core::error::{AppError, AppResult};
use fubbik_db::repo::chunk::{self, ListParams};
use fubbik_db::repo::space;
use fubbik_db::repo::vocabulary::{
    self, NewVocabularyEntry, NewVocabularyEntryItem, VocabularyEntry, VocabularyPatch,
};
use sqlx::PgPool;

use super::dto::Category;
use super::parser::{self, ParseResult, VocabEntry};
use super::suggest::{self, SuggestedEntry};

/// Mirrors Node's `verifySpaceOwnership` (`service.ts:19-26`): 404s as
/// `"Space"` when the space doesn't exist or isn't the caller's.
async fn verify_space_ownership(pool: &PgPool, user_id: &str, space_id: &str) -> AppResult<()> {
    space::find_by_id(pool, user_id, space_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Space".into()))?;
    Ok(())
}

pub async fn list_vocabulary(
    pool: &PgPool,
    user_id: &str,
    space_id: &str,
) -> AppResult<Vec<VocabularyEntry>> {
    verify_space_ownership(pool, user_id, space_id).await?;
    vocabulary::list(pool, user_id, space_id).await
}

/// Auto-seeds the 16 standard modifiers the first time an entry is created
/// in a space (`service.ts:39-43`), then inserts the entry itself.
pub async fn create_entry(
    pool: &PgPool,
    user_id: &str,
    word: &str,
    category: Category,
    expects: Option<Vec<String>>,
    space_id: &str,
) -> AppResult<VocabularyEntry> {
    verify_space_ownership(pool, user_id, space_id).await?;

    let count = vocabulary::count(pool, user_id, space_id).await?;
    if count == 0 {
        vocabulary::seed_modifiers(pool, user_id, space_id).await?;
    }

    let id = fubbik_db::new_id();
    vocabulary::create_entry(
        pool,
        user_id,
        NewVocabularyEntry {
            id,
            word: word.to_string(),
            category: category.as_str().to_string(),
            expects,
            space_id: space_id.to_string(),
        },
    )
    .await?
    .ok_or_else(|| AppError::NotFound("Space".into()))
}

pub async fn create_entries(
    pool: &PgPool,
    user_id: &str,
    entries: Vec<(String, Category, Option<Vec<String>>)>,
    space_id: &str,
) -> AppResult<Vec<VocabularyEntry>> {
    verify_space_ownership(pool, user_id, space_id).await?;

    let items = entries
        .into_iter()
        .map(|(word, category, expects)| NewVocabularyEntryItem {
            id: fubbik_db::new_id(),
            word,
            category: category.as_str().to_string(),
            expects,
        })
        .collect();

    vocabulary::create_entries(pool, user_id, space_id, items).await
}

pub async fn update_entry(
    pool: &PgPool,
    user_id: &str,
    id: &str,
    word: Option<String>,
    category: Option<Category>,
    expects: Option<Vec<String>>,
) -> AppResult<VocabularyEntry> {
    let entry = vocabulary::get_by_id(pool, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Vocabulary entry".into()))?;
    verify_space_ownership(pool, user_id, &entry.space_id).await?;

    vocabulary::update(
        pool,
        user_id,
        id,
        VocabularyPatch {
            word,
            category: category.map(|c| c.as_str().to_string()),
            expects,
        },
    )
    .await?
    .ok_or_else(|| AppError::NotFound("Vocabulary entry".into()))
}

pub async fn delete_entry(pool: &PgPool, user_id: &str, id: &str) -> AppResult<()> {
    let entry = vocabulary::get_by_id(pool, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Vocabulary entry".into()))?;
    verify_space_ownership(pool, user_id, &entry.space_id).await?;

    if vocabulary::delete(pool, user_id, id).await? {
        Ok(())
    } else {
        Err(AppError::NotFound("Vocabulary entry".into()))
    }
}

pub async fn parse_step(
    pool: &PgPool,
    user_id: &str,
    text: &str,
    space_id: &str,
) -> AppResult<ParseResult> {
    verify_space_ownership(pool, user_id, space_id).await?;
    let vocab = vocabulary::list(pool, user_id, space_id).await?;
    let entries: Vec<VocabEntry> = vocab
        .into_iter()
        .map(|v| VocabEntry {
            word: v.word,
            category: v.category,
            expects: v.expects.map(|j| j.0),
        })
        .collect();
    Ok(parser::parse_step_text(text, &entries))
}

/// Feeds the space's 50 most-recently-relevant chunks (Node's
/// `listChunks({ userId, spaceId, limit: 50, offset: 0 })`,
/// `service.ts:113-118`) to Ollama for vocabulary extraction. Always
/// succeeds — see `suggest::suggest_vocabulary`'s doc comment.
pub async fn suggest_from_chunks(
    pool: &PgPool,
    user_id: &str,
    space_id: &str,
) -> AppResult<Vec<SuggestedEntry>> {
    verify_space_ownership(pool, user_id, space_id).await?;

    let params = ListParams {
        space_id: Some(space_id.to_string()),
        limit: 50,
        offset: 0,
        ..Default::default()
    };
    let chunks = chunk::list(pool, user_id, &params).await?;
    let pairs: Vec<(String, String)> = chunks.into_iter().map(|c| (c.title, c.content)).collect();

    Ok(suggest::suggest_vocabulary(&pairs, None).await)
}
