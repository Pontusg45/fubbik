//! One key-value pattern, three scopes.
//!
//! `user_settings` (owned by `user_id`), `codebase_settings` (owned by
//! `space_id` — **the table name predates the codebase->space rename**;
//! there is no `space_settings` table and none should be added), and
//! `instance_settings` (global, no owner column at all — `key` is its
//! primary key). All three store an arbitrary `jsonb` `value` under a
//! `key`, faithfully reproducing Node's `t.Unknown()` body schema
//! (`packages/api/src/settings/routes.ts`) — there is no shape validation
//! on `value` anywhere in this module, matching Node exactly. A client can
//! `PATCH` `theme` to `42` and this stores it as-is, just like Node does.
//!
//! Every list function below orders by `key ASC` alone, with **no `id`
//! tiebreaker** — and this is not an oversight. Each function's `WHERE`
//! clause already narrows to a single scope (one `user_id`, one
//! `space_id`, or the whole global table for `instance_settings`), and
//! every scope carries a `UNIQUE` index on `(scope, key)`
//! (`user_settings_user_key_idx`, `codebase_settings_cb_key_idx`; for
//! `instance_settings`, `key` itself is the table's primary key). That
//! makes `key` already unique *within the result set* of any of these
//! queries, so `ORDER BY key` is a total order on its own — no two rows in
//! one query's result can ever tie on `key`. Contrast `favorite::list` /
//! `notification::list`, which tie-break with `id ASC` because their sort
//! column (`"order"`, `created_at`) genuinely can repeat within a scope.
//!
//! `codebase_settings`'s read and write both carry an `EXISTS (SELECT 1
//! FROM space s WHERE s.id = $.. AND s.user_id = $..)` ownership guard —
//! scoping through the parent `space` row the way `chunk_applies_to` /
//! `chunk_file_ref` scope through their parent `chunk`
//! (`chunk_meta::get_applies_to` et al.). This is a **deliberate
//! divergence from Node**: Node's `codebase_settings` repo functions
//! (`packages/db/src/repository/settings.ts`) take a bare `spaceId` with
//! no ownership check at all. The divergence is required by the task
//! brief, not a silent addition — "writing settings on another user's
//! space must be impossible" — and is proven load-bearing in
//! `tests/settings.rs`: remove either `EXISTS` clause and
//! `cannot_write_codebase_setting_for_another_users_space` /
//! `cannot_read_codebase_settings_for_another_users_space` fail.
//!
//! **These two guards are masked at the HTTP layer** by
//! `fubbik_api::settings::service`'s own ownership pre-check
//! (`space::find_by_id` before either repo call) — with a repo-level
//! `EXISTS` clause deleted, `fubbik-api/tests/settings.rs`'s equivalent
//! cross-user tests keep passing, because the service layer already 404s
//! before this module is ever called. Only the repo-level tests in this
//! crate catch a regression here; that is a property of this specific
//! layering (the same shape the `favorites` review flagged: a
//! service-level pre-check can mask a removed SQL guard from every test
//! above it), not a reason either guard is redundant — the repo functions
//! are `pub` and reachable by any future caller that skips the service
//! layer's pre-check, so both guards stay.

use fubbik_core::error::AppResult;
use sqlx::PgPool;
use sqlx::types::Json;

use crate::timestamp::UtcTimestamp;

// --- User settings ---

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UserSetting {
    pub id: String,
    pub user_id: String,
    pub key: String,
    #[schema(value_type = serde_json::Value)]
    pub value: Json<serde_json::Value>,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub updated_at: UtcTimestamp,
}

/// Scoped by `user_id` in SQL — never left to the caller.
pub async fn list_user_settings(pool: &PgPool, user_id: &str) -> AppResult<Vec<UserSetting>> {
    let rows = sqlx::query_as!(
        UserSetting,
        r#"SELECT id, user_id, key, value AS "value: Json<serde_json::Value>",
                  updated_at AS "updated_at: UtcTimestamp"
           FROM user_settings
           WHERE user_id = $1
           ORDER BY key ASC"#,
        user_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Upsert on the `(user_id, key)` unique index, matching Node's
/// `.onConflictDoUpdate({ target: [userSettings.userId, userSettings.key], ... })`.
pub async fn set_user_setting(
    pool: &PgPool,
    id: &str,
    user_id: &str,
    key: &str,
    value: serde_json::Value,
) -> AppResult<UserSetting> {
    let row = sqlx::query_as!(
        UserSetting,
        r#"INSERT INTO user_settings (id, user_id, key, value)
           VALUES ($1, $2, $3, $4)
           ON CONFLICT (user_id, key) DO UPDATE
             SET value = EXCLUDED.value, updated_at = now()
           RETURNING id, user_id, key, value AS "value: Json<serde_json::Value>",
                     updated_at AS "updated_at: UtcTimestamp""#,
        id,
        user_id,
        key,
        Json(value) as _
    )
    .fetch_one(pool)
    .await?;
    Ok(row)
}

// --- Codebase (space) settings ---

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CodebaseSetting {
    pub id: String,
    pub space_id: String,
    pub key: String,
    #[schema(value_type = serde_json::Value)]
    pub value: Json<serde_json::Value>,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub updated_at: UtcTimestamp,
}

/// Scoped by `space_id` *and* an `EXISTS` ownership guard through the
/// parent `space` row — see the module doc for why this guard exists at
/// all when Node's equivalent has none. A `space_id` that exists but
/// belongs to another user returns an empty list here, same as a
/// nonexistent `space_id`; the caller (`settings::service::get_all_codebase_settings`)
/// disambiguates with its own pre-check to produce a clean 404 instead of
/// a silent empty `{}`.
pub async fn list_codebase_settings(
    pool: &PgPool,
    space_id: &str,
    user_id: &str,
) -> AppResult<Vec<CodebaseSetting>> {
    let rows = sqlx::query_as!(
        CodebaseSetting,
        r#"SELECT id, space_id, key, value AS "value: Json<serde_json::Value>",
                  updated_at AS "updated_at: UtcTimestamp"
           FROM codebase_settings
           WHERE space_id = $1
             AND EXISTS (SELECT 1 FROM space s WHERE s.id = $1 AND s.user_id = $2)
           ORDER BY key ASC"#,
        space_id,
        user_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Upsert on the `(space_id, key)` unique index, guarded by the same
/// `EXISTS` ownership check as `list_codebase_settings`, expressed as an
/// `INSERT ... SELECT ... WHERE EXISTS (...)` so that a non-owned
/// `space_id` inserts (and updates) nothing at all — not even a row that
/// then gets rolled back, the row is never proposed to `ON CONFLICT` in
/// the first place. Returns `None` in that case, exactly like
/// `favorite::add`'s ownership guard on `chunk_id`.
pub async fn set_codebase_setting(
    pool: &PgPool,
    id: &str,
    space_id: &str,
    user_id: &str,
    key: &str,
    value: serde_json::Value,
) -> AppResult<Option<CodebaseSetting>> {
    let row = sqlx::query_as!(
        CodebaseSetting,
        r#"INSERT INTO codebase_settings (id, space_id, key, value)
           SELECT $1, $2, $3, $4
           WHERE EXISTS (SELECT 1 FROM space s WHERE s.id = $2 AND s.user_id = $5)
           ON CONFLICT (space_id, key) DO UPDATE
             SET value = EXCLUDED.value, updated_at = now()
           RETURNING id, space_id, key, value AS "value: Json<serde_json::Value>",
                     updated_at AS "updated_at: UtcTimestamp""#,
        id,
        space_id,
        key,
        Json(value) as _,
        user_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

// --- Instance settings ---

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct InstanceSetting {
    pub key: String,
    #[schema(value_type = serde_json::Value)]
    pub value: Json<serde_json::Value>,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub updated_at: UtcTimestamp,
}

/// Global by design — no scoping parameter at all. `instance_settings` has
/// no `user_id` column and no per-caller ownership concept.
pub async fn list_instance_settings(pool: &PgPool) -> AppResult<Vec<InstanceSetting>> {
    let rows = sqlx::query_as!(
        InstanceSetting,
        r#"SELECT key, value AS "value: Json<serde_json::Value>",
                  updated_at AS "updated_at: UtcTimestamp"
           FROM instance_settings
           ORDER BY key ASC"#
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Upsert keyed on `instance_settings.key` (the table's primary key, no
/// composite scoping), matching Node's `.onConflictDoUpdate({ target:
/// instanceSettings.key, ... })`.
pub async fn set_instance_setting(
    pool: &PgPool,
    key: &str,
    value: serde_json::Value,
) -> AppResult<InstanceSetting> {
    let row = sqlx::query_as!(
        InstanceSetting,
        r#"INSERT INTO instance_settings (key, value)
           VALUES ($1, $2)
           ON CONFLICT (key) DO UPDATE
             SET value = EXCLUDED.value, updated_at = now()
           RETURNING key, value AS "value: Json<serde_json::Value>",
                     updated_at AS "updated_at: UtcTimestamp""#,
        key,
        Json(value) as _
    )
    .fetch_one(pool)
    .await?;
    Ok(row)
}
