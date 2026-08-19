//! The `connection_relation` catalog — the set of labels a chunk-to-chunk
//! edge may carry (`chunk_connection.relation` is a FK to
//! `connection_relation(id)` with `ON DELETE RESTRICT`,
//! `migrations/0001_init.sql:2182`).
//!
//! A **separate table** from `chunk_type`, not the same table with a
//! discriminator — see `crate::repo::chunk_type`'s module doc for the
//! evidence. Guard shape is identical to that module's; only the columns
//! differ (`arrow_style`, `direction`, `inverse_of_id` here;
//! `icon`/`examples` there).
//!
//! Thirteen built-in relations are seeded by
//! `migrations/0002_seed_reference_data.sql:40-73` with `built_in = true`
//! and `user_id IS NULL`, and are rejected for edit/delete by the service
//! layer exactly as Node does
//! (`packages/api/src/vocabularies/service.ts:118-150`).

use fubbik_core::error::AppResult;
use sqlx::PgPool;

use crate::timestamp::UtcTimestamp;

/// `camelCase` serialisation matches every other wire type in this crate.
///
/// `arrow_style` and `direction` are stored as plain `text` with column
/// defaults and **no** CHECK constraint (`migrations/0001_init.sql:357-358`),
/// so they stay `String` on the read path — the same call this port made for
/// `vocabulary_entry.category`. They *are* constrained on the *input* side
/// (Node declares real `t.Union` literals in
/// `packages/api/src/vocabularies/routes.ts:30-31`), which is modelled by
/// the enums in `fubbik_api::vocabularies::dto`, not here.
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionRelation {
    pub id: String,
    pub label: String,
    pub description: Option<String>,
    pub arrow_style: String,
    pub direction: String,
    pub color: String,
    pub inverse_of_id: Option<String>,
    pub display_order: i32,
    pub built_in: bool,
    pub user_id: Option<String>,
    pub space_id: Option<String>,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub created_at: UtcTimestamp,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub updated_at: UtcTimestamp,
}

/// Insert payload. `id` is caller-supplied (a slug) and is the primary key.
pub struct NewConnectionRelation {
    pub id: String,
    pub label: String,
    pub description: Option<String>,
    pub arrow_style: Option<String>,
    pub direction: Option<String>,
    pub color: Option<String>,
    pub inverse_of_id: Option<String>,
    pub display_order: Option<i32>,
}

/// Built-in rows plus anything scoped to this user — and, when `space_id`
/// is supplied, anything scoped to that space.
///
/// Same `OR` (not `AND`) semantics as `chunk_type::list`: Node's
/// `listConnectionRelations`
/// (`packages/db/src/repository/vocabulary-catalog.ts:31-38`) widens rather
/// than filters when `spaceId` is passed. Reproduced verbatim; flagged in
/// `fubbik_api::vocabularies::service`.
///
/// `ORDER BY display_order ASC, id ASC` is Node's own
/// (`vocabulary-catalog.ts:38`), tiebreaker included.
pub async fn list(
    pool: &PgPool,
    user_id: &str,
    space_id: Option<&str>,
) -> AppResult<Vec<ConnectionRelation>> {
    let rows = sqlx::query_as!(
        ConnectionRelation,
        r#"SELECT id, label, description, arrow_style, direction, color,
                  inverse_of_id, display_order, built_in, user_id, space_id,
                  created_at AS "created_at: UtcTimestamp",
                  updated_at AS "updated_at: UtcTimestamp"
           FROM connection_relation
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

/// Unscoped by design, matching Node's `findConnectionRelationById`
/// (`packages/db/src/repository/vocabulary-catalog.ts:147-152`) — see
/// `chunk_type::find_by_id`'s doc comment for why that is safe here.
pub async fn find_by_id(pool: &PgPool, id: &str) -> AppResult<Option<ConnectionRelation>> {
    let row = sqlx::query_as!(
        ConnectionRelation,
        r#"SELECT id, label, description, arrow_style, direction, color,
                  inverse_of_id, display_order, built_in, user_id, space_id,
                  created_at AS "created_at: UtcTimestamp",
                  updated_at AS "updated_at: UtcTimestamp"
           FROM connection_relation WHERE id = $1"#,
        id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Always creates a non-built-in, user-owned row — Node hardcodes
/// `builtIn: false` (`packages/db/src/repository/vocabulary-catalog.ts:138`).
///
/// Defaults are applied here rather than deferred to the column defaults
/// for the same reason as `chunk_type::create`: Node's `displayOrder ?? 500`
/// (`vocabulary-catalog.ts:137`) disagrees with the column's `DEFAULT 100`
/// (`migrations/0001_init.sql:361`).
///
/// `space_id` is always `NULL` — Node's route body carries no `spaceId`
/// field (`packages/api/src/vocabularies/routes.ts:26-35`).
pub async fn create(
    pool: &PgPool,
    user_id: &str,
    new: NewConnectionRelation,
) -> AppResult<ConnectionRelation> {
    let row = sqlx::query_as!(
        ConnectionRelation,
        r#"INSERT INTO connection_relation
             (id, label, description, arrow_style, direction, color, inverse_of_id,
              display_order, built_in, user_id, space_id)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, false, $9, NULL)
           RETURNING id, label, description, arrow_style, direction, color,
                     inverse_of_id, display_order, built_in, user_id, space_id,
                     created_at AS "created_at: UtcTimestamp",
                     updated_at AS "updated_at: UtcTimestamp""#,
        new.id,
        new.label,
        new.description,
        new.arrow_style.unwrap_or_else(|| "solid".to_string()),
        new.direction.unwrap_or_else(|| "forward".to_string()),
        new.color.unwrap_or_else(|| "#64748b".to_string()),
        new.inverse_of_id,
        new.display_order.unwrap_or(500),
        user_id
    )
    .fetch_one(pool)
    .await?;
    Ok(row)
}

/// Patch payload. `description` and `inverse_of_id` are tri-state
/// (`Some(None)` clears, `None` leaves untouched) because Node's PATCH
/// schema gives them a `t.Null()` variant and its repo forwards an explicit
/// `null` through (`packages/db/src/repository/vocabulary-catalog.ts:158-165`).
#[derive(Default)]
pub struct ConnectionRelationPatch {
    pub label: Option<String>,
    pub description: Option<Option<String>>,
    pub arrow_style: Option<String>,
    pub direction: Option<String>,
    pub color: Option<String>,
    pub inverse_of_id: Option<Option<String>>,
    pub display_order: Option<i32>,
}

/// `WHERE id = $1 AND user_id = $2` — the SQL-level ownership guard, and
/// the only guard Node's `updateConnectionRelationRow` has
/// (`packages/db/src/repository/vocabulary-catalog.ts:167`). Built-in rows
/// have `user_id IS NULL`, so this blocks them too.
pub async fn update(
    pool: &PgPool,
    user_id: &str,
    id: &str,
    patch: ConnectionRelationPatch,
) -> AppResult<Option<ConnectionRelation>> {
    let row = sqlx::query_as!(
        ConnectionRelation,
        r#"UPDATE connection_relation SET
             label = COALESCE($3, label),
             description = CASE WHEN $4 THEN $5 ELSE description END,
             arrow_style = COALESCE($6, arrow_style),
             direction = COALESCE($7, direction),
             color = COALESCE($8, color),
             inverse_of_id = CASE WHEN $9 THEN $10 ELSE inverse_of_id END,
             display_order = COALESCE($11, display_order),
             updated_at = now()
           WHERE id = $1 AND user_id = $2
           RETURNING id, label, description, arrow_style, direction, color,
                     inverse_of_id, display_order, built_in, user_id, space_id,
                     created_at AS "created_at: UtcTimestamp",
                     updated_at AS "updated_at: UtcTimestamp""#,
        id,
        user_id,
        patch.label,
        patch.description.is_some(),
        patch.description.flatten(),
        patch.arrow_style,
        patch.direction,
        patch.color,
        patch.inverse_of_id.is_some(),
        patch.inverse_of_id.flatten(),
        patch.display_order
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// `WHERE id = $1 AND user_id = $2`, matching Node's
/// `deleteConnectionRelationRow`
/// (`packages/db/src/repository/vocabulary-catalog.ts:173-181`) — no
/// `built_in = false` clause, because Node has none.
pub async fn delete(pool: &PgPool, user_id: &str, id: &str) -> AppResult<bool> {
    let res = sqlx::query!(
        "DELETE FROM connection_relation WHERE id = $1 AND user_id = $2",
        id,
        user_id
    )
    .execute(pool)
    .await?;
    Ok(res.rows_affected() > 0)
}
