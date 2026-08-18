//! Controlled vocabulary for BDD-style requirement steps
//! (`packages/db/src/repository/vocabulary.ts`,
//! `packages/db/src/schema/vocabulary.ts`).
//!
//! `vocabulary_entry` is scoped to a `space`, not directly to a user:
//! `space_id` is `NOT NULL ... ON DELETE CASCADE`, but `user_id` is
//! nullable with `ON DELETE SET NULL` — the same shape the Phase 2e brief
//! flags for `tag_type`: a user-created row whose owner column can go
//! `NULL` out from under it, so `user_id` cannot be the authorization
//! boundary. Node's own service (`packages/api/src/vocabulary/service.ts`)
//! never checks `entry.userId` either — every operation authorizes via
//! `verifySpaceOwnership(spaceId, userId)`, i.e. "does the *space* this
//! entry lives in belong to the caller". This module reproduces that same
//! boundary in SQL: every function that takes a `space_id` directly
//! (`list`, `count`, `create_entry`, `create_entries`, `seed_modifiers`)
//! carries an `EXISTS (SELECT 1 FROM space ...)` ownership guard, and
//! `update`/`delete` (which take an entry `id`) join back to `space`
//! through the row's own `space_id` — see each function's doc comment for
//! why this is defense-in-depth on top of, not a replacement for, the
//! service-layer `verify_space_ownership` pre-check (same two-layer shape
//! as `settings::get_all_codebase_settings` / `set_codebase_setting`).
//!
//! `category` is `text NOT NULL` with **no** Postgres check constraint or
//! enum type (`packages/db/src/migrations/0000_baseline.sql:356`) — the
//! only constraint on it is Elysia's `t.Union` of six literals in the
//! route schema (`packages/api/src/vocabulary/routes.ts:7-14`). That *is*
//! a genuine input constraint (unlike, say, `notification.type`, which has
//! no constraint anywhere), so the API layer models it as an enum
//! (`fubbik_api::vocabulary::dto::Category`) — but this repo module keeps
//! `category` a plain `String` end to end, matching what Node's `SELECT *`
//! actually returns and round-trips.
//!
//! `expects` (jsonb, nullable, no default) is **never** validated against
//! the category enum — Node's route schema is `t.Optional(t.Array(t.String()))`,
//! plain strings, not `t.Array(CategorySchema)`. Modelling it as anything
//! but `Vec<String>` would reject data Node accepts.
//!
//! `definition` (`text`, nullable) exists in the schema but no code path
//! in this domain ever sets it — every insert/update here omits it, same
//! as Node. It stays in the row shape (and round-trips as `null`) because
//! `SELECT *`/`db.select()` in Node returns every column.

use fubbik_core::error::AppResult;
use sqlx::PgPool;
use sqlx::types::Json;

use crate::timestamp::UtcTimestamp;

/// `camelCase` serialisation matches every other wire type in this crate.
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct VocabularyEntry {
    pub id: String,
    pub word: String,
    pub definition: Option<String>,
    pub category: String,
    #[schema(value_type = Option<Vec<String>>)]
    pub expects: Option<Json<Vec<String>>>,
    pub space_id: String,
    pub user_id: Option<String>,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub created_at: UtcTimestamp,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub updated_at: UtcTimestamp,
}

/// Lists a space's vocabulary. Node's `listVocabulary(spaceId)`
/// (`packages/db/src/repository/vocabulary.ts:7-15`) has no ownership
/// guard of its own — it relies entirely on the service layer's
/// `verifySpaceOwnership` pre-check. This adds an `EXISTS` guard as a
/// second, independent layer: see
/// `tests/vocabulary.rs::list_returns_empty_for_a_space_the_caller_does_not_own_even_bypassing_the_service_check`
/// for proof this guard alone (with the service-layer check hypothetically
/// removed) still keeps another user's vocabulary from leaking.
///
/// `ORDER BY category, word` matches Node's `orderBy(asc(category),
/// asc(word))` exactly; `, id ASC` is an added tiebreaker — Node has none,
/// but ties on `(category, word)` are possible (the unique index is on
/// `(space_id, category, lower(word))`, and two entries can share a
/// `category`/`word` pair differing only in case before the
/// `lower(word)` uniqueness applies)... in practice the unique index makes
/// `(category, word)` ties rare, but a total order is required by this
/// port's own conventions regardless — see `chunk::list`'s equivalent
/// comment.
pub async fn list(pool: &PgPool, user_id: &str, space_id: &str) -> AppResult<Vec<VocabularyEntry>> {
    let rows = sqlx::query_as!(
        VocabularyEntry,
        r#"SELECT id, word, definition, category,
                  expects AS "expects: Json<Vec<String>>",
                  space_id, user_id,
                  created_at AS "created_at: UtcTimestamp",
                  updated_at AS "updated_at: UtcTimestamp"
           FROM vocabulary_entry
           WHERE space_id = $1
             AND EXISTS (SELECT 1 FROM space s WHERE s.id = $1 AND s.user_id = $2)
           ORDER BY category ASC, word ASC, id ASC"#,
        space_id,
        user_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Backs `createEntry`'s auto-seed check
/// (`packages/api/src/vocabulary/service.ts:40-43`): `count === 0` decides
/// whether to seed the standard modifiers first. Same `EXISTS` guard shape
/// as `list`.
pub async fn count(pool: &PgPool, user_id: &str, space_id: &str) -> AppResult<i64> {
    let count = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "count!" FROM vocabulary_entry
           WHERE space_id = $1
             AND EXISTS (SELECT 1 FROM space s WHERE s.id = $1 AND s.user_id = $2)"#,
        space_id,
        user_id
    )
    .fetch_one(pool)
    .await?;
    Ok(count)
}

pub struct NewVocabularyEntry {
    pub id: String,
    pub word: String,
    pub category: String,
    pub expects: Option<Vec<String>>,
    pub space_id: String,
}

/// Creates one entry, lower-casing `word` the same way Node's
/// `createVocabularyEntry` does (`params.word.toLowerCase()`). Guarded by
/// an `INSERT ... SELECT ... WHERE EXISTS (...)` — a foreign `space_id`
/// inserts nothing and this returns `None`, the same shape as
/// `collection::create`'s space guard.
pub async fn create_entry(
    pool: &PgPool,
    user_id: &str,
    new: NewVocabularyEntry,
) -> AppResult<Option<VocabularyEntry>> {
    let word_lower = new.word.to_lowercase();
    let row = sqlx::query_as!(
        VocabularyEntry,
        r#"INSERT INTO vocabulary_entry (id, word, category, expects, space_id, user_id)
           SELECT $1, $2, $3, $4, $5, $6
           WHERE EXISTS (SELECT 1 FROM space s WHERE s.id = $5 AND s.user_id = $6)
           RETURNING id, word, definition, category,
                     expects AS "expects: Json<Vec<String>>",
                     space_id, user_id,
                     created_at AS "created_at: UtcTimestamp",
                     updated_at AS "updated_at: UtcTimestamp""#,
        new.id,
        word_lower,
        new.category,
        new.expects.map(Json) as _,
        new.space_id,
        user_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub struct NewVocabularyEntryItem {
    pub id: String,
    pub word: String,
    pub category: String,
    pub expects: Option<Vec<String>>,
}

/// Bulk-creates entries in one space. Node's `createVocabularyEntries`
/// (`packages/db/src/repository/vocabulary.ts:43-70`) is a single
/// multi-row `INSERT ... ON CONFLICT DO NOTHING RETURNING`, not wrapped in
/// a transaction; this issues one `INSERT ... ON CONFLICT DO NOTHING
/// RETURNING` per row inside a transaction instead (sqlx's `query_as!`
/// macro needs a fixed column list per statement, so a single dynamic
/// multi-row insert would need runtime `QueryBuilder` construction for
/// little benefit at this domain's scale) — strictly more atomic than
/// Node, never less. The space ownership `EXISTS` check runs once up front
/// (not per row): a foreign `space_id` short-circuits to `Ok(vec![])`
/// before any row is attempted, matching `create_entry`'s guard semantics
/// without repeating the subquery N times.
pub async fn create_entries(
    pool: &PgPool,
    user_id: &str,
    space_id: &str,
    entries: Vec<NewVocabularyEntryItem>,
) -> AppResult<Vec<VocabularyEntry>> {
    if entries.is_empty() {
        return Ok(Vec::new());
    }

    let owned = sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM space s WHERE s.id = $1 AND s.user_id = $2) AS "owned!""#,
        space_id,
        user_id
    )
    .fetch_one(pool)
    .await?;
    if !owned {
        return Ok(Vec::new());
    }

    let mut tx = pool.begin().await?;
    let mut created = Vec::with_capacity(entries.len());
    for entry in entries {
        let word_lower = entry.word.to_lowercase();
        let row = sqlx::query_as!(
            VocabularyEntry,
            r#"INSERT INTO vocabulary_entry (id, word, category, expects, space_id, user_id)
               VALUES ($1, $2, $3, $4, $5, $6)
               ON CONFLICT DO NOTHING
               RETURNING id, word, definition, category,
                         expects AS "expects: Json<Vec<String>>",
                         space_id, user_id,
                         created_at AS "created_at: UtcTimestamp",
                         updated_at AS "updated_at: UtcTimestamp""#,
            entry.id,
            word_lower,
            entry.category,
            entry.expects.map(Json) as _,
            space_id,
            user_id
        )
        .fetch_optional(&mut *tx)
        .await?;
        if let Some(row) = row {
            created.push(row);
        }
    }
    tx.commit().await?;
    Ok(created)
}

/// The 16 standard modifier words Node auto-seeds into a space the first
/// time an entry is created in it
/// (`packages/db/src/repository/vocabulary.ts:101`). Order matches Node's
/// array exactly (irrelevant to behaviour — `ON CONFLICT DO NOTHING` makes
/// insertion order a non-issue — but kept identical for easy diffing).
const STANDARD_MODIFIERS: [&str; 16] = [
    "a", "an", "the", "is", "are", "was", "were", "with", "on", "to", "their", "not", "has",
    "have", "they", "it",
];

/// Same guard shape as `create_entries`: one ownership check up front, then
/// one `INSERT ... ON CONFLICT DO NOTHING RETURNING` per modifier inside a
/// transaction.
pub async fn seed_modifiers(
    pool: &PgPool,
    user_id: &str,
    space_id: &str,
) -> AppResult<Vec<VocabularyEntry>> {
    let owned = sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM space s WHERE s.id = $1 AND s.user_id = $2) AS "owned!""#,
        space_id,
        user_id
    )
    .fetch_one(pool)
    .await?;
    if !owned {
        return Ok(Vec::new());
    }

    let mut tx = pool.begin().await?;
    let mut created = Vec::with_capacity(STANDARD_MODIFIERS.len());
    for word in STANDARD_MODIFIERS {
        let id = crate::new_id();
        let row = sqlx::query_as!(
            VocabularyEntry,
            r#"INSERT INTO vocabulary_entry (id, word, category, expects, space_id, user_id)
               VALUES ($1, $2, 'modifier', NULL, $3, $4)
               ON CONFLICT DO NOTHING
               RETURNING id, word, definition, category,
                         expects AS "expects: Json<Vec<String>>",
                         space_id, user_id,
                         created_at AS "created_at: UtcTimestamp",
                         updated_at AS "updated_at: UtcTimestamp""#,
            id,
            word,
            space_id,
            user_id
        )
        .fetch_optional(&mut *tx)
        .await?;
        if let Some(row) = row {
            created.push(row);
        }
    }
    tx.commit().await?;
    Ok(created)
}

/// Bare lookup by id, no ownership scoping at all — matches Node's
/// `getVocabularyEntry(id)` (`packages/db/src/repository/vocabulary.ts:94-99`)
/// exactly. This is safe: callers (`update_entry`/`delete_entry` in
/// `fubbik_api::vocabulary::service`) only ever use the returned row's
/// `space_id` to run `verify_space_ownership` before doing anything
/// user-visible with it — the row itself is never handed back to an
/// unauthorized caller. The actual authorization boundary for
/// update/delete is the `EXISTS` guard in `update`/`delete` below, not
/// this function.
pub async fn get_by_id(pool: &PgPool, id: &str) -> AppResult<Option<VocabularyEntry>> {
    let row = sqlx::query_as!(
        VocabularyEntry,
        r#"SELECT id, word, definition, category,
                  expects AS "expects: Json<Vec<String>>",
                  space_id, user_id,
                  created_at AS "created_at: UtcTimestamp",
                  updated_at AS "updated_at: UtcTimestamp"
           FROM vocabulary_entry WHERE id = $1"#,
        id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Two-state (`None` = leave untouched, `Some` = set) on all three fields —
/// Node's PATCH body (`t.Optional(t.String())` / `t.Optional(CategorySchema)`
/// / `t.Optional(t.Array(t.String()))`, `packages/api/src/vocabulary/routes.ts:124-128`)
/// has no `t.Null()` variant anywhere, so there is no way for a client to
/// explicitly clear any of these fields through this endpoint.
#[derive(Default)]
pub struct VocabularyPatch {
    pub word: Option<String>,
    pub category: Option<String>,
    pub expects: Option<Vec<String>>,
}

/// Node's `updateVocabularyEntry` always issues the `UPDATE` — even an
/// all-omitted patch — because `vocabularyEntry.updatedAt` has a Drizzle
/// `$onUpdate` hook that fires on any `.set(...)` call
/// (`packages/db/src/schema/vocabulary.ts:24-26`); this mirrors that with
/// an unconditional `updated_at = now()`. `word`, when provided, is
/// lower-cased before the `COALESCE`, matching
/// `params.word.toLowerCase()`.
///
/// The `EXISTS (... vocabulary_entry.space_id ...)` guard is
/// defense-in-depth: Node's own `updateVocabularyEntry` has no such check
/// at all (`WHERE id = $1` only) — it relies entirely on the service
/// layer's `getVocabularyEntry` + `verifySpaceOwnership` sequence run
/// *before* this is ever called. This guard proves that boundary still
/// holds even if that service-layer check were ever removed — see
/// `tests/vocabulary.rs::update_cannot_touch_another_users_entry_via_a_guessed_id_even_bypassing_the_service_check`.
pub async fn update(
    pool: &PgPool,
    user_id: &str,
    id: &str,
    patch: VocabularyPatch,
) -> AppResult<Option<VocabularyEntry>> {
    let word_lower = patch.word.map(|w| w.to_lowercase());
    let row = sqlx::query_as!(
        VocabularyEntry,
        r#"UPDATE vocabulary_entry SET
             word = COALESCE($3, word),
             category = COALESCE($4, category),
             expects = COALESCE($5, expects),
             updated_at = now()
           WHERE id = $1
             AND EXISTS (SELECT 1 FROM space s WHERE s.id = vocabulary_entry.space_id AND s.user_id = $2)
           RETURNING id, word, definition, category,
                     expects AS "expects: Json<Vec<String>>",
                     space_id, user_id,
                     created_at AS "created_at: UtcTimestamp",
                     updated_at AS "updated_at: UtcTimestamp""#,
        id,
        user_id,
        word_lower,
        patch.category,
        patch.expects.map(Json) as _
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Same defense-in-depth `EXISTS` guard as `update` — Node's
/// `deleteVocabularyEntry(id)` is a bare `WHERE id = $1` relying entirely
/// on the service layer's pre-check.
pub async fn delete(pool: &PgPool, user_id: &str, id: &str) -> AppResult<bool> {
    let res = sqlx::query!(
        r#"DELETE FROM vocabulary_entry
           WHERE id = $1
             AND EXISTS (SELECT 1 FROM space s WHERE s.id = vocabulary_entry.space_id AND s.user_id = $2)"#,
        id,
        user_id
    )
    .execute(pool)
    .await?;
    Ok(res.rows_affected() > 0)
}
