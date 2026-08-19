//! The `chunk_type` catalog — the set of kinds a chunk's `type` column may
//! hold (`chunk.type` is a FK to `chunk_type(id)` with `ON DELETE RESTRICT`,
//! `migrations/0001_init.sql`).
//!
//! **`chunk_type` and `connection_relation` are two separate tables**, not
//! one table with a discriminator: `migrations/0001_init.sql:283` and
//! `:353` declare them independently, with different column sets
//! (`icon`/`examples` only on `chunk_type`; `arrow_style`/`direction`/
//! `inverse_of_id` only on `connection_relation`). Hence two repo modules
//! and two `ToSchema` row types, sharing only their shape of guard logic.
//!
//! **Built-in rows are seeded by migration and read-only.** Seven built-in
//! chunk types (`note`, `document`, `guide`, `reference`, `schema`,
//! `checklist`, `convention`) land via `migrations/0002_seed_reference_data.sql:15-38`
//! with `built_in = true` and `user_id IS NULL`. Node's
//! `updateChunkType`/`deleteChunkType`
//! (`packages/api/src/vocabularies/service.ts:55-87`) load the row
//! *unscoped*, reject `builtIn` rows with a `ValidationError` (400), and
//! only then run the scoped mutation. That service-layer check is
//! replicated in `fubbik_api::vocabularies::service`.
//!
//! Unlike `template::delete`, Node's vocabulary-catalog `UPDATE`/`DELETE`
//! carry **no** explicit `built_in = false` clause
//! (`packages/db/src/repository/vocabulary-catalog.ts:96,106`) — only
//! `WHERE id = $1 AND user_id = $2`. This port matches that exactly. The
//! `user_id` guard alone still makes a real built-in row unmutable, because
//! every seeded built-in has `user_id IS NULL` and no caller's `user_id`
//! can equal `NULL`.

use fubbik_core::error::AppResult;
use sqlx::PgPool;
use sqlx::types::Json;

use crate::timestamp::UtcTimestamp;

/// `camelCase` serialisation matches every other wire type in this crate.
/// Node returns the whole Drizzle row from `db.select().from(chunkType)`
/// (`packages/db/src/repository/vocabulary-catalog.ts:21-26`), so every
/// column is on the wire — including `builtIn`, `userId`, `spaceId` and
/// both timestamps.
///
/// `examples` is `jsonb NOT NULL DEFAULT '[]'` holding a plain string
/// array (`packages/db/src/schema/chunk-type.ts:22`).
///
/// `color` looks constrained (a hex string, `maxLength: 9` in Elysia) but
/// has no CHECK constraint and no enum anywhere — plain `String`.
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChunkType {
    pub id: String,
    pub label: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub color: String,
    #[schema(value_type = Vec<String>)]
    pub examples: Json<Vec<String>>,
    pub display_order: i32,
    pub built_in: bool,
    pub user_id: Option<String>,
    pub space_id: Option<String>,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub created_at: UtcTimestamp,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub updated_at: UtcTimestamp,
}

/// Insert payload. `id` is caller-supplied (a slug), not generated — the
/// primary key of this table is the slug itself.
pub struct NewChunkType {
    pub id: String,
    pub label: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub examples: Option<Vec<String>>,
    pub display_order: Option<i32>,
}

/// Built-in rows plus anything scoped to this user — and, when `space_id`
/// is supplied, anything scoped to that space.
///
/// Note the **`OR`**: Node's `listChunkTypes` builds
/// `or(builtIn = true, userId = ..., spaceId = ...)`
/// (`packages/db/src/repository/vocabulary-catalog.ts:18-25`), so passing
/// `spaceId` *widens* the result set rather than narrowing it. That is
/// reproduced verbatim here; see `fubbik_api::vocabularies::service`'s note
/// flagging it as suspect Node behaviour rather than a deliberate design.
///
/// `ORDER BY display_order ASC, id ASC` is Node's own ordering
/// (`vocabulary-catalog.ts:25`) — not an addition by this port. `id ASC` is
/// Node's explicit tiebreaker for the (very common) case of equal
/// `display_order`.
pub async fn list(
    pool: &PgPool,
    user_id: &str,
    space_id: Option<&str>,
) -> AppResult<Vec<ChunkType>> {
    let rows = sqlx::query_as!(
        ChunkType,
        r#"SELECT id, label, description, icon, color,
                  examples AS "examples: Json<Vec<String>>",
                  display_order, built_in, user_id, space_id,
                  created_at AS "created_at: UtcTimestamp",
                  updated_at AS "updated_at: UtcTimestamp"
           FROM chunk_type
           WHERE built_in = true
              OR user_id = $1
              OR ($2::text IS NOT NULL AND space_id = $2)
           ORDER BY display_order ASC, id ASC"#,
        user_id,
        space_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Unscoped by design, matching Node's `findChunkTypeById`
/// (`packages/db/src/repository/vocabulary-catalog.ts:77-82`): `WHERE id =
/// $1` only. The service layer uses it purely to answer "does this slug
/// already exist?" and "is it built-in?" before attempting a scoped
/// mutation; no field from this lookup is returned to the caller on the
/// mutation paths, so it cannot leak another user's row content. The
/// ownership guard lives in `update`/`delete` below.
pub async fn find_by_id(pool: &PgPool, id: &str) -> AppResult<Option<ChunkType>> {
    let row = sqlx::query_as!(
        ChunkType,
        r#"SELECT id, label, description, icon, color,
                  examples AS "examples: Json<Vec<String>>",
                  display_order, built_in, user_id, space_id,
                  created_at AS "created_at: UtcTimestamp",
                  updated_at AS "updated_at: UtcTimestamp"
           FROM chunk_type WHERE id = $1"#,
        id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Always creates a non-built-in, user-owned row — Node hardcodes
/// `builtIn: false` (`packages/db/src/repository/vocabulary-catalog.ts:68`).
/// There is no path through this API that creates a built-in chunk type.
///
/// The three defaults are applied here in Rust rather than left to the
/// column defaults because Node applies them in *its* repo layer with `??`
/// and one of them disagrees with the schema: `displayOrder ?? 500`
/// (`vocabulary-catalog.ts:67`) vs. the column's `DEFAULT 100`
/// (`migrations/0001_init.sql:290`). Deferring to Postgres would silently
/// change the value every custom type gets. `color`/`examples` happen to
/// match their column defaults, but are applied the same way for symmetry.
///
/// `space_id` is always `NULL`: Node's route body schema
/// (`packages/api/src/vocabularies/routes.ts:7-15`) has no `spaceId` or
/// `codebaseId` field at all, so `createChunkType`'s `codebaseId` service
/// parameter is unreachable — and even if set, the repo reads `row.spaceId`,
/// not `row.codebaseId`. See the note in
/// `fubbik_api::vocabularies::service::create_chunk_type`.
pub async fn create(pool: &PgPool, user_id: &str, new: NewChunkType) -> AppResult<ChunkType> {
    let row = sqlx::query_as!(
        ChunkType,
        r#"INSERT INTO chunk_type
             (id, label, description, icon, color, examples, display_order, built_in, user_id, space_id)
           VALUES ($1, $2, $3, $4, $5, $6, $7, false, $8, NULL)
           RETURNING id, label, description, icon, color,
                     examples AS "examples: Json<Vec<String>>",
                     display_order, built_in, user_id, space_id,
                     created_at AS "created_at: UtcTimestamp",
                     updated_at AS "updated_at: UtcTimestamp""#,
        new.id,
        new.label,
        new.description,
        new.icon,
        new.color.unwrap_or_else(|| "#8b5cf6".to_string()),
        Json(new.examples.unwrap_or_default()) as _,
        new.display_order.unwrap_or(500),
        user_id
    )
    .fetch_one(pool)
    .await?;
    Ok(row)
}

/// Patch payload. `description`/`icon` are tri-state (`Some(None)` clears
/// the column, `None` leaves it untouched) because Node's PATCH schema
/// declares them `t.Union([t.String(...), t.Null()])` and its repo forwards
/// an explicit `null` through via `data.field !== undefined`
/// (`packages/db/src/repository/vocabulary-catalog.ts:88-94`). Every other
/// field has no `t.Null()` variant, so plain `Option<T>` is correct.
#[derive(Default)]
pub struct ChunkTypePatch {
    pub label: Option<String>,
    pub description: Option<Option<String>>,
    pub icon: Option<Option<String>>,
    pub color: Option<String>,
    pub examples: Option<Vec<String>>,
    pub display_order: Option<i32>,
}

/// `WHERE id = $1 AND user_id = $2` — the SQL-level ownership guard, and
/// the *only* guard Node's `updateChunkTypeRow` has
/// (`packages/db/src/repository/vocabulary-catalog.ts:96`). A built-in
/// row's `user_id` is `NULL`, which no real caller's `user_id` can equal,
/// so this also blocks mutation of built-ins independently of the
/// service-layer `built_in` pre-check.
///
/// `updated_at = now()` reproduces Drizzle's `$onUpdate`
/// (`packages/db/src/schema/chunk-type.ts:29-32`), which fires on every
/// `.set()`.
pub async fn update(
    pool: &PgPool,
    user_id: &str,
    id: &str,
    patch: ChunkTypePatch,
) -> AppResult<Option<ChunkType>> {
    let row = sqlx::query_as!(
        ChunkType,
        r#"UPDATE chunk_type SET
             label = COALESCE($3, label),
             description = CASE WHEN $4 THEN $5 ELSE description END,
             icon = CASE WHEN $6 THEN $7 ELSE icon END,
             color = COALESCE($8, color),
             examples = COALESCE($9, examples),
             display_order = COALESCE($10, display_order),
             updated_at = now()
           WHERE id = $1 AND user_id = $2
           RETURNING id, label, description, icon, color,
                     examples AS "examples: Json<Vec<String>>",
                     display_order, built_in, user_id, space_id,
                     created_at AS "created_at: UtcTimestamp",
                     updated_at AS "updated_at: UtcTimestamp""#,
        id,
        user_id,
        patch.label,
        patch.description.is_some(),
        patch.description.flatten(),
        patch.icon.is_some(),
        patch.icon.flatten(),
        patch.color,
        patch.examples.map(Json) as _,
        patch.display_order
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// `WHERE id = $1 AND user_id = $2`, matching Node's `deleteChunkTypeRow`
/// (`packages/db/src/repository/vocabulary-catalog.ts:102-110`) — note
/// there is deliberately **no** `AND built_in = false` here, because Node
/// has none either (unlike `template::delete`, which does).
pub async fn delete(pool: &PgPool, user_id: &str, id: &str) -> AppResult<bool> {
    let res = sqlx::query!(
        "DELETE FROM chunk_type WHERE id = $1 AND user_id = $2",
        id,
        user_id
    )
    .execute(pool)
    .await?;
    Ok(res.rows_affected() > 0)
}
