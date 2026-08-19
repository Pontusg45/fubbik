//! Feature overlays: named entities that carry *sparse* field-level deltas
//! against chunks, resolved on read in priority order.
//!
//! Ports Node's two repository files at once — `packages/db/src/repository/
//! feature.ts` (the `feature` / `feature_space` / `user_active_feature`
//! side) and `packages/db/src/repository/chunk-feature-delta.ts` (the
//! `chunk_feature_delta` side). They are one module here because every
//! delta query joins `feature` for its `priority`, which is the only thing
//! that makes a delta resolvable at all.
//!
//! # Scoping
//!
//! Node's repository functions in this domain take a `userId` in only four
//! places (`getFeatureById`, `listFeatures`, `updateFeature`,
//! `deleteFeature`, `featureNameConflict`, `getMaxPriority`,
//! `getActiveFeatureIds`, `setActiveFeatures`). Everything else —
//! `getSpacesForFeature`, `setFeatureSpaces`, `getDeltasForChunk`,
//! `getDeltasForFeature`, `upsertDelta`, `deleteDelta` — takes a bare id
//! and relies entirely on the *service* layer having checked ownership
//! first. `GET /chunks/{id}/deltas` is the case where that reliance
//! actually fails: `packages/api/src/features/routes.ts:123-125` calls
//! `featureService.getDeltasForChunk(ctx.params.id)` with the session
//! **discarded**, so any authenticated user can read the overlay content of
//! any chunk by id.
//!
//! **Every query in this module carries its `user_id` filter in the SQL
//! itself**, through the parent `feature` or `chunk` row where the table has
//! no `user_id` column of its own — the same "prove it in SQL" pattern as
//! `workspace::add_space`/`remove_space` and `chunk_version::list_for_chunk`.
//! That is a deliberate divergence from Node for the five unscoped
//! functions above; see the individual doc comments. The service layer
//! keeps its own pre-checks so the 404-vs-empty distinction Node produces
//! is preserved.
//!
//! # `priority` is unique per user
//!
//! `feature_user_priority_idx` is a `UNIQUE INDEX` on `(user_id, priority)`
//! (migration `0001_init.sql:1769`), and `feature_user_name_idx` likewise on
//! `(user_id, name)`. Neither is deferrable, so Postgres checks them per
//! row, mid-statement. See [`shift_priorities_up`] for the Node bug this
//! exposes.

use fubbik_core::error::AppResult;
use sqlx::PgPool;
use sqlx::types::Json;

use crate::timestamp::UtcTimestamp;

/// Full `feature` row — what `POST /features`, `GET /features/{id}`'s
/// `feature` key, `PATCH /features/{id}` and `POST /features/{id}/reorder`
/// all return. Includes `userId`, which the *list* projection deliberately
/// omits; see [`FeatureListItem`].
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Feature {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub priority: i32,
    /// Free text at the database level — the column is a plain `text` with
    /// a `'inactive'` default and **no CHECK constraint** (migration
    /// `0001_init.sql:411`). Node's route schema constrains what a *client*
    /// may PATCH (`active | inactive | archived`) but `merged` is written
    /// only by the merge path, and `POST /features` cannot set it at all.
    /// Modelled as `String`, not an enum, so the wire shape can never be
    /// narrower than the column.
    pub status: String,
    /// Also free text: `t.String({ maxLength: 7 })` in Node's route schema,
    /// with no format validation anywhere. Not an enum, not validated.
    pub color: Option<String>,
    pub user_id: String,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub created_at: UtcTimestamp,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub updated_at: UtcTimestamp,
}

/// `GET /features` row. Deliberately **not** [`Feature`]: Node's
/// `listFeatures` hand-picks columns and adds an aggregate
/// (`packages/db/src/repository/feature.ts:42-58`), so `userId` is absent
/// and `deltaCount` is present. Reusing `Feature` here would publish a
/// list shape that leaks `userId` and drops `deltaCount`.
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FeatureListItem {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub priority: i32,
    pub status: String,
    pub color: Option<String>,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub created_at: UtcTimestamp,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub updated_at: UtcTimestamp,
    /// `count(chunk_feature_delta.id)::int` — Node casts to `int`, so this
    /// is `i32`, not `i64`.
    pub delta_count: i32,
}

/// The `{ id, name }` space projection returned inside `GET /features/{id}`
/// — Node's `getSpacesForFeature` selects exactly those two columns
/// (`packages/db/src/repository/feature.ts:117-125`). Named `FeatureSpace`
/// rather than `Space` because `repo::space::Space` already owns that
/// schema name and this is a *different, narrower* shape.
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct FeatureSpace {
    pub id: String,
    pub name: String,
}

/// Bare `chunk_feature_delta` row — the return shape of
/// `PUT /chunks/{id}/deltas/{featureId}` and
/// `DELETE /chunks/{id}/deltas/{featureId}`'s repository call.
///
/// `delta` is a **sparse** JSON object holding only the changed fields
/// (`title`, `content`, `type`, `rationale`, `alternatives`,
/// `consequences`, `summary`) — never a whole chunk. Typed as a raw
/// `serde_json::Value` for exactly that reason: a struct with seven
/// `Option` fields would serialise absent keys back as explicit `null`s and
/// destroy the sparseness on the next round-trip.
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChunkFeatureDelta {
    pub id: String,
    pub chunk_id: String,
    pub feature_id: String,
    #[schema(value_type = serde_json::Value)]
    pub delta: Json<serde_json::Value>,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub created_at: UtcTimestamp,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub updated_at: UtcTimestamp,
}

/// `GET /chunks/{id}/deltas` row: a delta plus the four denormalised
/// `feature` columns Node's `getDeltasForChunk` joins in
/// (`packages/db/src/repository/chunk-feature-delta.ts:23-43`).
/// `featurePriority` is what the client sorts by to resolve conflicts.
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeltaWithFeature {
    pub id: String,
    pub chunk_id: String,
    pub feature_id: String,
    #[schema(value_type = serde_json::Value)]
    pub delta: Json<serde_json::Value>,
    pub feature_name: String,
    pub feature_priority: i32,
    pub feature_color: Option<String>,
    pub feature_status: String,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub created_at: UtcTimestamp,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub updated_at: UtcTimestamp,
}

/// `GET /features/{id}/deltas` (and the `deltas` key of
/// `GET /features/{id}`) row: a delta plus the owning chunk's title, per
/// Node's `getDeltasForFeature`
/// (`packages/db/src/repository/chunk-feature-delta.ts:45-61`).
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeltaWithChunk {
    pub id: String,
    pub chunk_id: String,
    pub feature_id: String,
    #[schema(value_type = serde_json::Value)]
    pub delta: Json<serde_json::Value>,
    pub chunk_title: String,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub created_at: UtcTimestamp,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub updated_at: UtcTimestamp,
}

// ---------------------------------------------------------------------------
// feature CRUD
// ---------------------------------------------------------------------------

/// Inserts a feature. `priority` is computed by the caller (either supplied
/// verbatim by the client or `max_priority + 1`) — this function does not
/// resolve collisions, matching Node. A collision with an existing
/// `(user_id, priority)` pair raises a unique violation here exactly as it
/// does in Node, surfacing as a 500; see this module's header.
pub async fn create(
    pool: &PgPool,
    id: &str,
    user_id: &str,
    name: &str,
    description: Option<&str>,
    priority: i32,
    color: Option<&str>,
) -> AppResult<Feature> {
    let row = sqlx::query_as!(
        Feature,
        r#"INSERT INTO feature (id, name, description, priority, color, user_id)
           VALUES ($1, $3, $4, $5, $6, $2)
           RETURNING id, name, description, priority, status, color, user_id,
                     created_at AS "created_at: UtcTimestamp",
                     updated_at AS "updated_at: UtcTimestamp""#,
        id,
        user_id,
        name,
        description,
        priority,
        color
    )
    .fetch_one(pool)
    .await?;
    Ok(row)
}

/// `WHERE id = $1 AND user_id = $2`, matching Node's `getFeatureById`.
/// A feature belonging to someone else is indistinguishable from one that
/// does not exist — both are `None`, both become a 404.
pub async fn find_by_id(pool: &PgPool, id: &str, user_id: &str) -> AppResult<Option<Feature>> {
    let row = sqlx::query_as!(
        Feature,
        r#"SELECT id, name, description, priority, status, color, user_id,
                  created_at AS "created_at: UtcTimestamp",
                  updated_at AS "updated_at: UtcTimestamp"
           FROM feature WHERE id = $1 AND user_id = $2"#,
        id,
        user_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Filters for [`list`]. Every field is truthy-checked the way Node checks
/// them (`if (filters?.status)`, `if (filters?.search)`,
/// `if (!filters?.codebaseId) return features`), so an **empty string means
/// "no filter"**, not "match the empty string".
#[derive(Default)]
pub struct ListParams<'a> {
    pub space_id: Option<&'a str>,
    pub status: Option<&'a str>,
    pub search: Option<&'a str>,
}

/// Lists a user's features with their delta counts.
///
/// Three things are ported precisely:
///
/// - **The space filter is not a simple join.** Node fetches the features
///   linked to the requested space *and* every `feature_space` row in the
///   table, then keeps `f.id ∈ linkedToThisSpace || f.id ∉ anyLink`
///   (`packages/db/src/repository/feature.ts:60-71`). In other words a
///   feature with no space association at all is **global** and shows up
///   under every `spaceId`; only features linked to *some other* space are
///   hidden. The two `EXISTS`/`NOT EXISTS` subqueries below are that same
///   predicate — Node's `linkedSet` is built unscoped across all users, but
///   `f` is already restricted to `user_id = $1`, so "has any link" is
///   equivalent.
/// - **`search` is a raw `ILIKE '%…%'`** with no escaping of `%` or `_`,
///   matching Drizzle's `ilike(feature.name, `%${search}%`)`. A search for
///   `%` matches everything, in Node and here alike.
/// - **`ORDER BY priority ASC, id ASC`.** Node orders by `feature.priority`
///   alone. `(user_id, priority)` is unique, so within one user's rows that
///   is already a total order and the `id` tiebreaker can never fire — it
///   is appended only for consistency with every other `list` in this
///   crate. `tests/feature.rs::list_orders_by_priority` documents why no
///   tie can be constructed here.
pub async fn list(
    pool: &PgPool,
    user_id: &str,
    params: &ListParams<'_>,
) -> AppResult<Vec<FeatureListItem>> {
    let status = params.status.filter(|s| !s.is_empty());
    let search = params.search.filter(|s| !s.is_empty());
    let space_id = params.space_id.filter(|s| !s.is_empty());

    let rows = sqlx::query_as!(
        FeatureListItem,
        r#"SELECT f.id, f.name, f.description, f.priority, f.status, f.color,
                  f.created_at AS "created_at: UtcTimestamp",
                  f.updated_at AS "updated_at: UtcTimestamp",
                  count(d.id)::int AS "delta_count!"
           FROM feature f
           LEFT JOIN chunk_feature_delta d ON d.feature_id = f.id
           WHERE f.user_id = $1
             AND ($2::text IS NULL OR f.status = $2)
             AND ($3::text IS NULL OR f.name ILIKE '%' || $3 || '%')
             AND ($4::text IS NULL
                  OR EXISTS (SELECT 1 FROM feature_space fs
                             WHERE fs.feature_id = f.id AND fs.space_id = $4)
                  OR NOT EXISTS (SELECT 1 FROM feature_space fs
                                 WHERE fs.feature_id = f.id))
           GROUP BY f.id
           ORDER BY f.priority ASC, f.id ASC"#,
        user_id,
        status,
        search,
        space_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// `description` and `color` are tri-state (`None` = untouched,
/// `Some(None)` = clear, `Some(Some(v))` = set), matching Node's route
/// schema, which types both as `t.Optional(t.Union([t.String(), t.Null()]))`
/// (`packages/api/src/features/routes.ts:85,88`). `name`, `priority` and
/// `status` have no null variant there, so plain `Option<T>` (COALESCE) is
/// enough.
#[derive(Default)]
pub struct FeaturePatch {
    pub name: Option<String>,
    pub description: Option<Option<String>>,
    pub priority: Option<i32>,
    pub status: Option<String>,
    pub color: Option<Option<String>>,
}

/// Applies a partial update, scoped by `user_id` in the statement.
///
/// **Divergence from Node, deliberate.** Node calls Drizzle's
/// `.set(repoBody)` unconditionally; with an empty object Drizzle throws
/// `No values to set` before any SQL is issued, so a `PATCH` whose only key
/// is `spaceIds` — a perfectly ordinary request, since `spaceIds` is
/// stripped off before the repo call
/// (`packages/api/src/features/service.ts:112`) — becomes a **500** in Node.
/// This port short-circuits to a plain re-select instead, the same way
/// `requirement::update` and `use_case::update` do, so an all-omitted patch
/// is a 200 no-op and `updated_at` stays untouched.
pub async fn update(
    pool: &PgPool,
    id: &str,
    user_id: &str,
    patch: FeaturePatch,
) -> AppResult<Option<Feature>> {
    if patch.name.is_none()
        && patch.description.is_none()
        && patch.priority.is_none()
        && patch.status.is_none()
        && patch.color.is_none()
    {
        return find_by_id(pool, id, user_id).await;
    }

    let (description_set, description_val) = match patch.description {
        Some(v) => (true, v),
        None => (false, None),
    };
    let (color_set, color_val) = match patch.color {
        Some(v) => (true, v),
        None => (false, None),
    };

    let row = sqlx::query_as!(
        Feature,
        r#"UPDATE feature SET
             name = COALESCE($3, name),
             description = CASE WHEN $4 THEN $5 ELSE description END,
             priority = COALESCE($6, priority),
             status = COALESCE($7, status),
             color = CASE WHEN $8 THEN $9 ELSE color END,
             updated_at = now()
           WHERE id = $1 AND user_id = $2
           RETURNING id, name, description, priority, status, color, user_id,
                     created_at AS "created_at: UtcTimestamp",
                     updated_at AS "updated_at: UtcTimestamp""#,
        id,
        user_id,
        patch.name,
        description_set,
        description_val,
        patch.priority,
        patch.status,
        color_set,
        color_val
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Deletes and returns the removed row (Node returns the deleted feature so
/// the service can 404 on `null`). `chunk_feature_delta`, `feature_space`
/// and `user_active_feature` all cascade off `feature.id`.
pub async fn delete(pool: &PgPool, id: &str, user_id: &str) -> AppResult<Option<Feature>> {
    let row = sqlx::query_as!(
        Feature,
        r#"DELETE FROM feature WHERE id = $1 AND user_id = $2
           RETURNING id, name, description, priority, status, color, user_id,
                     created_at AS "created_at: UtcTimestamp",
                     updated_at AS "updated_at: UtcTimestamp""#,
        id,
        user_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// `true` when another feature of the same user already carries `name`.
/// The `id <> $1` exclusion is what makes a rename-to-itself legal.
pub async fn name_conflict(pool: &PgPool, id: &str, user_id: &str, name: &str) -> AppResult<bool> {
    let hit = sqlx::query_scalar!(
        r#"SELECT EXISTS (
             SELECT 1 FROM feature
             WHERE user_id = $2 AND name = $3 AND id <> $1
           ) AS "exists!""#,
        id,
        user_id,
        name
    )
    .fetch_one(pool)
    .await?;
    Ok(hit)
}

/// `coalesce(max(priority), 0)` for a user — the base for auto-assigned
/// priorities (`max + 1`), so the first feature a user creates gets `1`.
pub async fn max_priority(pool: &PgPool, user_id: &str) -> AppResult<i32> {
    let max = sqlx::query_scalar!(
        r#"SELECT coalesce(max(priority), 0)::int AS "max!" FROM feature WHERE user_id = $1"#,
        user_id
    )
    .fetch_one(pool)
    .await?;
    Ok(max)
}

/// Increments `priority` for every feature of `user_id` at or above
/// `new_priority`, opening a slot at `new_priority`. Ports Node's
/// `shiftPriorities(userId, newPriority, "up")`
/// (`packages/db/src/repository/feature.ts:127-141`) verbatim — including
/// the fact that it is a single bulk `UPDATE`.
///
/// **KNOWN NODE BUG, reproduced deliberately.** `feature_user_priority_idx`
/// is a non-deferrable `UNIQUE INDEX` on `(user_id, priority)`, so Postgres
/// validates it per row *during* the statement. Shifting a contiguous run
/// therefore collides with itself: with priorities `1,2,3` and
/// `new_priority = 1`, updating the `1` row to `2` hits the still-present
/// `2` row and the statement aborts with
/// `duplicate key value violates unique constraint`. Verified directly
/// against Postgres 18 before writing this port. The fix would be an
/// ordered descending update or a deferrable constraint — both change
/// observable behaviour, so neither is applied here. See
/// `tests/feature.rs::shift_priorities_up_hits_nodes_unique_violation`.
pub async fn shift_priorities_up(pool: &PgPool, user_id: &str, new_priority: i32) -> AppResult<()> {
    sqlx::query!(
        "UPDATE feature SET priority = priority + 1, updated_at = now()
         WHERE user_id = $1 AND priority >= $2",
        user_id,
        new_priority
    )
    .execute(pool)
    .await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// feature_space
// ---------------------------------------------------------------------------

/// Replaces a feature's space associations wholesale.
///
/// Two hardenings over Node's `setFeatureSpaces(featureId, spaceIds)`
/// (`packages/db/src/repository/feature.ts:106-115`), which takes no
/// `userId` at all:
///
/// - **Both sides are ownership-checked in SQL.** The `DELETE` only fires
///   for a feature the caller owns, and the `INSERT` only links spaces the
///   caller owns — the same shape as `workspace::add_space`'s
///   `w.user_id = $1 AND s.user_id = $1` join. Node's version happily links
///   your feature to another user's space (the FK to `space(id)` is the
///   only check there is) and, called with someone else's `featureId`,
///   would wipe their associations.
/// - **One transaction.** Node issues the delete and the insert as two
///   independent statements, so a failed insert leaves the feature with no
///   spaces at all.
///
/// A `space_id` the caller does not own is silently skipped rather than
/// raising — same "no per-entry error" convention as `favorite::reorder`.
pub async fn set_spaces(
    pool: &PgPool,
    feature_id: &str,
    user_id: &str,
    space_ids: &[String],
) -> AppResult<()> {
    let mut tx = pool.begin().await?;
    sqlx::query!(
        "DELETE FROM feature_space
         WHERE feature_id = $1
           AND EXISTS (SELECT 1 FROM feature f WHERE f.id = $1 AND f.user_id = $2)",
        feature_id,
        user_id
    )
    .execute(&mut *tx)
    .await?;

    if !space_ids.is_empty() {
        sqlx::query!(
            "INSERT INTO feature_space (feature_id, space_id)
             SELECT f.id, s.id
             FROM feature f
             JOIN space s ON s.id = ANY($3)
             WHERE f.id = $1 AND f.user_id = $2 AND s.user_id = $2
             ON CONFLICT DO NOTHING",
            feature_id,
            user_id,
            space_ids
        )
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

/// The `{ id, name }` spaces attached to a feature, scoped through the
/// feature's owner. Node's `getSpacesForFeature` takes no `userId`; the
/// `EXISTS` clause is this port's SQL-level guard. **No `ORDER BY`**,
/// matching Node exactly — following the same rule as
/// `requirement::get_chunks` and `coverage`'s three queries, the other
/// ports of Node queries with no ordering at all.
pub async fn spaces_for_feature(
    pool: &PgPool,
    feature_id: &str,
    user_id: &str,
) -> AppResult<Vec<FeatureSpace>> {
    let rows = sqlx::query_as!(
        FeatureSpace,
        "SELECT s.id, s.name
         FROM feature_space fs
         JOIN space s ON fs.space_id = s.id
         WHERE fs.feature_id = $1
           AND EXISTS (SELECT 1 FROM feature f WHERE f.id = $1 AND f.user_id = $2)",
        feature_id,
        user_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

// ---------------------------------------------------------------------------
// user_active_feature
// ---------------------------------------------------------------------------

/// The feature ids this user has switched on. **No `ORDER BY`**, matching
/// Node's `getActiveFeatureIds`.
pub async fn active_feature_ids(pool: &PgPool, user_id: &str) -> AppResult<Vec<String>> {
    let rows = sqlx::query_scalar!(
        "SELECT feature_id FROM user_active_feature WHERE user_id = $1",
        user_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Replaces the active set, delete-then-insert inside one transaction.
///
/// The `INSERT ... SELECT ... WHERE f.user_id = $1` is the SQL-level
/// ownership guard: an id belonging to another user is dropped rather than
/// linked. Node's `setActiveFeatures` inserts whatever ids it is handed —
/// only the *service* rejects foreign ids, with a `ValidationError`. That
/// service check is kept (it is what produces the 400 Node's own route test
/// asserts, `packages/api/src/features/routes.test.ts:136-141`), so this
/// guard is defence in depth, not the primary path.
pub async fn set_active_features(
    pool: &PgPool,
    user_id: &str,
    feature_ids: &[String],
) -> AppResult<()> {
    let mut tx = pool.begin().await?;
    sqlx::query!(
        "DELETE FROM user_active_feature WHERE user_id = $1",
        user_id
    )
    .execute(&mut *tx)
    .await?;

    if !feature_ids.is_empty() {
        sqlx::query!(
            "INSERT INTO user_active_feature (user_id, feature_id)
             SELECT $1, f.id FROM feature f
             WHERE f.id = ANY($2) AND f.user_id = $1
             ON CONFLICT DO NOTHING",
            user_id,
            feature_ids
        )
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// chunk_feature_delta
// ---------------------------------------------------------------------------

/// Inserts or replaces the single delta for a `(chunk, feature)` pair.
///
/// The `chunk_feature_delta_chunk_feature_idx` unique index is what makes
/// this an upsert; `ON CONFLICT ... DO UPDATE` reproduces Drizzle's
/// `onConflictDoUpdate({ target: [chunkId, featureId], set: { delta,
/// updatedAt } })`. `delta` is replaced wholesale, never merged — a caller
/// sending `{title}` after `{content}` ends up with only `title`, exactly
/// as in Node.
///
/// Returns `None` when the chunk or the feature is not the caller's (the
/// `INSERT ... SELECT` join is the guard — Node's `upsertDelta` takes no
/// `userId` and relies purely on its service pre-checks). The service still
/// pre-checks both so it can tell a missing *feature* from a missing
/// *chunk* in the 404 message, the same disambiguation `favorites::service`
/// does.
pub async fn upsert_delta(
    pool: &PgPool,
    id: &str,
    chunk_id: &str,
    feature_id: &str,
    user_id: &str,
    delta: &serde_json::Value,
) -> AppResult<Option<ChunkFeatureDelta>> {
    let row = sqlx::query_as!(
        ChunkFeatureDelta,
        r#"INSERT INTO chunk_feature_delta (id, chunk_id, feature_id, delta)
           SELECT $1, c.id, f.id, $5
           FROM chunk c
           JOIN feature f ON f.id = $3
           WHERE c.id = $2 AND c.user_id = $4 AND f.user_id = $4
           ON CONFLICT (chunk_id, feature_id)
             DO UPDATE SET delta = EXCLUDED.delta, updated_at = now()
           RETURNING id, chunk_id, feature_id,
                     delta AS "delta: Json<serde_json::Value>",
                     created_at AS "created_at: UtcTimestamp",
                     updated_at AS "updated_at: UtcTimestamp""#,
        id,
        chunk_id,
        feature_id,
        user_id,
        delta
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Every feature delta touching one chunk, newest-priority-last.
///
/// **This is the query Node leaves completely unscoped** — `GET
/// /chunks/{id}/deltas` discards the session
/// (`packages/api/src/features/routes.ts:123-125`), so in Node any signed-in
/// user can read any chunk's overlay text by guessing an id. The
/// `EXISTS (... chunk c WHERE c.id = $1 AND c.user_id = $2)` clause below
/// closes that: a foreign chunk yields an empty list, never its rows. This
/// is a deliberate divergence, flagged rather than silent.
///
/// `ORDER BY f.priority ASC, d.id ASC` — Node orders by `feature.priority`
/// alone. Unlike [`list`], a tie here is genuinely reachable (two features
/// owned by *different* users can both hold priority 1 and both carry a
/// delta on the same chunk), so the `id` tiebreaker is load-bearing rather
/// than decorative; see
/// `tests/feature.rs::deltas_for_chunk_breaks_priority_ties_by_id`.
/// Ascending priority is what makes the result directly foldable in
/// resolution order — see `resolve` in `packages/api/src/features/resolve.ts`,
/// which sorts ascending and `Object.assign`s in sequence, so the **highest**
/// priority is applied last and wins a same-field conflict.
pub async fn deltas_for_chunk(
    pool: &PgPool,
    chunk_id: &str,
    user_id: &str,
) -> AppResult<Vec<DeltaWithFeature>> {
    let rows = sqlx::query_as!(
        DeltaWithFeature,
        r#"SELECT d.id, d.chunk_id, d.feature_id,
                  d.delta AS "delta: Json<serde_json::Value>",
                  f.name AS feature_name, f.priority AS feature_priority,
                  f.color AS feature_color, f.status AS feature_status,
                  d.created_at AS "created_at: UtcTimestamp",
                  d.updated_at AS "updated_at: UtcTimestamp"
           FROM chunk_feature_delta d
           JOIN feature f ON d.feature_id = f.id
           WHERE d.chunk_id = $1
             AND EXISTS (SELECT 1 FROM chunk c WHERE c.id = $1 AND c.user_id = $2)
           ORDER BY f.priority ASC, d.id ASC"#,
        chunk_id,
        user_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Every delta belonging to one feature, with its chunk's title. Scoped
/// through the feature's owner in SQL (Node's `getDeltasForFeature` takes
/// only a `featureId`). **No `ORDER BY`**, matching Node exactly.
pub async fn deltas_for_feature(
    pool: &PgPool,
    feature_id: &str,
    user_id: &str,
) -> AppResult<Vec<DeltaWithChunk>> {
    let rows = sqlx::query_as!(
        DeltaWithChunk,
        r#"SELECT d.id, d.chunk_id, d.feature_id,
                  d.delta AS "delta: Json<serde_json::Value>",
                  c.title AS chunk_title,
                  d.created_at AS "created_at: UtcTimestamp",
                  d.updated_at AS "updated_at: UtcTimestamp"
           FROM chunk_feature_delta d
           JOIN chunk c ON d.chunk_id = c.id
           WHERE d.feature_id = $1
             AND EXISTS (SELECT 1 FROM feature f WHERE f.id = $1 AND f.user_id = $2)"#,
        feature_id,
        user_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Removes one `(chunk, feature)` delta, scoped through the feature's owner
/// in SQL. Returns the deleted row so the service can 404 on `None`,
/// matching Node's `deleteDelta` → `NotFoundError({ resource: "Delta" })`.
pub async fn delete_delta(
    pool: &PgPool,
    chunk_id: &str,
    feature_id: &str,
    user_id: &str,
) -> AppResult<Option<ChunkFeatureDelta>> {
    let row = sqlx::query_as!(
        ChunkFeatureDelta,
        r#"DELETE FROM chunk_feature_delta
           WHERE chunk_id = $1 AND feature_id = $2
             AND EXISTS (SELECT 1 FROM feature f WHERE f.id = $2 AND f.user_id = $3)
           RETURNING id, chunk_id, feature_id,
                     delta AS "delta: Json<serde_json::Value>",
                     created_at AS "created_at: UtcTimestamp",
                     updated_at AS "updated_at: UtcTimestamp""#,
        chunk_id,
        feature_id,
        user_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// The seven chunk fields a delta is allowed to carry. Kept here next to
/// the `UPDATE` in [`merge_feature_deltas`] that has to name every one of
/// them, so the two can never drift; the service re-exports it for
/// validation. Mirrors `DELTA_ALLOWED_FIELDS` in
/// `packages/api/src/features/service.ts:31`.
pub const DELTA_ALLOWED_FIELDS: [&str; 7] = [
    "title",
    "content",
    "type",
    "rationale",
    "alternatives",
    "consequences",
    "summary",
];

/// Permanently folds every delta of a feature into its base chunks, in one
/// transaction.
///
/// Per delta, in Node's order (`packages/db/src/repository/
/// chunk-feature-delta.ts:99-149`): read the base chunk, skip it if gone,
/// append a `chunk_version` snapshot of its *pre-merge* state, then apply
/// the delta. Afterwards, delete all of the feature's deltas and set its
/// `status = 'merged'`. Returns the ids of the chunks actually touched.
///
/// **Atomicity is the whole point.** Node wrapped this in
/// `db.transaction` after an earlier phase found the multi-write
/// non-atomic, and that property is load-bearing: a failure part-way
/// through would otherwise leave some chunks rewritten, their deltas
/// deleted, and the feature still unmerged — with no way to replay the
/// lost overlays. `tests/feature.rs::merge_is_atomic_under_forced_failure`
/// forces a mid-transaction failure (a second delta whose `title` is JSON
/// `null`, violating `chunk.title NOT NULL`) *after* the first chunk has
/// already been versioned and rewritten, and asserts that nothing at all
/// was written.
///
/// Three faithful details worth naming:
///
/// - **The version snapshot writes only `title`/`content`/`type`/`tags`.**
///   Node's insert names exactly those columns, leaving `rationale`,
///   `alternatives`, `consequences` and `scope` NULL in the history row —
///   unlike this crate's own `chunk_version::snapshot`, which carries
///   `rationale`/`consequences` through. Reproduced as Node has it rather
///   than "improved", because `GET /chunks/{id}/history` surfaces those
///   fields. `tags` is `'[]'` in Node too.
/// - **The delta is applied field-by-field from JSONB**, not as a
///   deserialised struct, so absent keys stay absent: `CASE WHEN
///   jsonb_exists(...)` reproduces Drizzle's `.set(delta)`, which only
///   emits assignments for the keys present. An explicit JSON `null` for a
///   `NOT NULL` column raises, in Node and here alike. `alternatives` is
///   extracted with `->` (it is a `jsonb` column); the other six with `->>`.
/// - **No re-enrichment.** Node fires `enrichChunk` per affected chunk
///   after the commit; there is no Ollama/enrichment service in this port
///   yet, so the returned ids are the hook for whenever there is one.
///
/// Guard note: the `AND user_id = $3` on the per-chunk `UPDATE` is the one
/// scoping clause in this module that could **not** be shown to be
/// load-bearing — deleting it leaves the whole repo suite green, because
/// the `SELECT ... AND user_id = $2` two statements earlier has already
/// `continue`d past any chunk the caller does not own, so the `UPDATE`
/// never sees one. It is kept as defence in depth (if that `SELECT` guard
/// were ever relaxed, this would still hold the line) and is documented
/// here rather than quietly presented as tested.
pub async fn merge_feature_deltas(
    pool: &PgPool,
    feature_id: &str,
    user_id: &str,
    deltas: &[(String, serde_json::Value)],
) -> AppResult<Vec<String>> {
    let mut tx = pool.begin().await?;
    let mut affected: Vec<String> = Vec::new();

    for (chunk_id, delta) in deltas {
        let existing = sqlx::query!(
            "SELECT title, content, type FROM chunk WHERE id = $1 AND user_id = $2",
            chunk_id,
            user_id
        )
        .fetch_optional(&mut *tx)
        .await?;
        // Node: `if (!existing) continue;`
        let Some(existing) = existing else { continue };

        let version_id = crate::new_id();
        sqlx::query!(
            "INSERT INTO chunk_version (id, chunk_id, version, title, content, type, tags)
             SELECT $1, $2, coalesce(max(v.version), 0) + 1, $3, $4, $5, '[]'::jsonb
             FROM chunk_version v WHERE v.chunk_id = $2",
            version_id,
            chunk_id,
            existing.title,
            existing.content,
            existing.r#type
        )
        .execute(&mut *tx)
        .await?;

        sqlx::query!(
            r#"UPDATE chunk SET
                 title = CASE WHEN jsonb_exists($2::jsonb, 'title')
                              THEN ($2::jsonb)->>'title' ELSE title END,
                 content = CASE WHEN jsonb_exists($2::jsonb, 'content')
                                THEN ($2::jsonb)->>'content' ELSE content END,
                 type = CASE WHEN jsonb_exists($2::jsonb, 'type')
                             THEN ($2::jsonb)->>'type' ELSE type END,
                 rationale = CASE WHEN jsonb_exists($2::jsonb, 'rationale')
                                  THEN ($2::jsonb)->>'rationale' ELSE rationale END,
                 alternatives = CASE WHEN jsonb_exists($2::jsonb, 'alternatives')
                                     THEN ($2::jsonb)->'alternatives' ELSE alternatives END,
                 consequences = CASE WHEN jsonb_exists($2::jsonb, 'consequences')
                                     THEN ($2::jsonb)->>'consequences' ELSE consequences END,
                 summary = CASE WHEN jsonb_exists($2::jsonb, 'summary')
                                THEN ($2::jsonb)->>'summary' ELSE summary END,
                 updated_at = now()
               WHERE id = $1 AND user_id = $3"#,
            chunk_id,
            delta,
            user_id
        )
        .execute(&mut *tx)
        .await?;

        affected.push(chunk_id.clone());
    }

    sqlx::query!(
        "DELETE FROM chunk_feature_delta
         WHERE feature_id = $1
           AND EXISTS (SELECT 1 FROM feature f WHERE f.id = $1 AND f.user_id = $2)",
        feature_id,
        user_id
    )
    .execute(&mut *tx)
    .await?;

    sqlx::query!(
        "UPDATE feature SET status = 'merged', updated_at = now()
         WHERE id = $1 AND user_id = $2",
        feature_id,
        user_id
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(affected)
}
