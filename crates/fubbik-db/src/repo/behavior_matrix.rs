//! Behavioral matrices: `behavior_matrix` and its seven satellite tables.
//!
//! A matrix holds `behavior_dimension` columns and `behavior_rule` rows;
//! their intersection is a `behavior_cell`. Cells carry evidence
//! (`behavior_cell_requirement`, `behavior_cell_code`,
//! `behavior_test_result`), and rules carry append-only history
//! (`behavior_rule_version`).
//!
//! # Ownership
//!
//! Only `behavior_matrix` has a `user_id`. Everything else derives ownership
//! from it, one to three hops away:
//!
//! ```text
//! matrix ──< dimension ──┐
//!    └────< rule ────────┴──< cell ──< {requirement, code, test_result}
//!                         └──< rule_version
//! ```
//!
//! **Every function here that takes a `user_id` enforces that chain in SQL**,
//! rather than trusting a service-layer pre-check. Node does the opposite —
//! its repository functions take bare ids and rely on the service having
//! called `getMatrixById` first, which for the entire cell surface it did not
//! (see the `fix(matrices)` commit: nine routes, six of them writes, with no
//! authorization at all). Putting the guard in the query means a future
//! caller that forgets the pre-check still cannot reach another user's data.
//!
//! Two Node functions are unscoped in a way this port deliberately does NOT
//! reproduce: `reorderDimensions`/`reorderRules` take a bare id list and
//! renumber whatever they name. The service checks that the *matrix* belongs
//! to the caller but never that the *ids* belong to that matrix, so a caller
//! owning any matrix can renumber another's rows. Here both are constrained
//! to the matrix in the UPDATE itself.

use fubbik_core::error::AppResult;
use sqlx::PgPool;
use sqlx::types::Json;

use crate::timestamp::UtcTimestamp;

// ---------------------------------------------------------------------------
// Row types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BehaviorMatrix {
    pub id: String,
    pub name: String,
    /// `invariant | contract`. Plain `text` with no CHECK — the constraint
    /// lives on the create route's schema only, so an older row may hold
    /// anything and the read path must not choke on it.
    pub layer: String,
    pub description: Option<String>,
    pub space_id: Option<String>,
    pub user_id: String,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub created_at: UtcTimestamp,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub updated_at: UtcTimestamp,
}

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BehaviorDimension {
    pub id: String,
    pub matrix_id: String,
    pub name: String,
    /// Quoted as `"order"` in every query — it is a reserved word in SQL.
    pub order: i32,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub created_at: UtcTimestamp,
}

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BehaviorRule {
    pub id: String,
    pub matrix_id: String,
    pub title: String,
    pub description: Option<String>,
    pub category: Option<String>,
    /// Decision context, mirroring `chunk`'s three fields. Note
    /// `alternatives` is **`text` here, not a jsonb array** — it shares a
    /// name with `chunk.alternatives` and not a type.
    pub rationale: Option<String>,
    pub alternatives: Option<String>,
    pub consequences: Option<String>,
    /// Explicit negative space: what violating this behaviour looks like.
    pub counterexample: Option<String>,
    pub order: i32,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub created_at: UtcTimestamp,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub updated_at: UtcTimestamp,
}

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BehaviorCell {
    pub id: String,
    pub rule_id: String,
    pub dimension_id: String,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub created_at: UtcTimestamp,
}

/// The seven fields Node snapshots into `behavior_rule_version.snapshot`
/// (`packages/db/src/schema/behavior-matrix.ts:76-84`) — the rule's editable
/// content, without its id/order/timestamps.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BehaviorRuleSnapshot {
    pub title: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub rationale: Option<String>,
    pub alternatives: Option<String>,
    pub consequences: Option<String>,
    pub counterexample: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BehaviorRuleVersion {
    pub id: String,
    pub rule_id: String,
    #[schema(value_type = BehaviorRuleSnapshot)]
    pub snapshot: Json<BehaviorRuleSnapshot>,
    /// `ON DELETE SET NULL`, not CASCADE — deleting a user must not erase the
    /// history of rules they edited.
    pub changed_by: Option<String>,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub created_at: UtcTimestamp,
}

/// `ref` is a Rust keyword, so the field is `code_ref` and renamed on the
/// wire and in SQL. The column itself is `ref`.
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BehaviorCellCode {
    pub id: String,
    pub cell_id: String,
    /// `file | symbol | test`, free `text` — constrained on the write route
    /// only. See the module doc on `layer`.
    pub kind: String,
    #[serde(rename = "ref")]
    pub code_ref: String,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub created_at: UtcTimestamp,
}

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BehaviorTestResult {
    pub id: String,
    pub cell_id: String,
    pub test_ref: String,
    /// `pass | fail`, free `text`.
    pub status: String,
    pub detail: Option<String>,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub run_at: UtcTimestamp,
}

/// One row of `GET /matrices/{id}/cells/{cellId}/requirements` — the link
/// plus the requirement's title and status.
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CellRequirement {
    pub requirement_id: String,
    pub title: String,
    pub status: String,
}

/// The `behavior_cell_requirement` join row itself, returned by the link
/// endpoint (Node returns the inserted row).
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CellRequirementLink {
    pub cell_id: String,
    pub requirement_id: String,
}

/// Per-cell aggregates backing the computed matrix view. `failing_count` is
/// requirements in `failing` status; the two test counts come from
/// `behavior_test_result`.
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MatrixViewCell {
    pub id: String,
    pub rule_id: String,
    pub dimension_id: String,
    pub requirement_count: i64,
    pub failing_count: i64,
    pub code_count: i64,
    pub passing_test_count: i64,
    pub failing_test_count: i64,
}

/// One row of the reverse lookup `GET /matrices/behaviors-for-file?path=`.
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BehaviorForFile {
    pub rule_id: String,
    pub rule_title: String,
    pub description: Option<String>,
    pub rationale: Option<String>,
    pub counterexample: Option<String>,
    pub matrix_id: String,
    pub matrix_name: String,
    pub layer: String,
    pub dimension_name: String,
    pub kind: String,
    #[serde(rename = "ref")]
    pub code_ref: String,
}

// ---------------------------------------------------------------------------
// Matrix CRUD
// ---------------------------------------------------------------------------

pub struct NewMatrix {
    pub name: String,
    pub layer: String,
    pub description: Option<String>,
    pub space_id: Option<String>,
}

/// Inserts a matrix. `space_id` is guarded at INSERT time via `WHERE EXISTS`,
/// the same accepted divergence `requirement::create` and `use_case::create`
/// carry: Node's `createMatrix` is a bare insert, so a foreign `spaceId`
/// there creates a matrix pointing at a space the caller cannot see. Here it
/// inserts nothing and returns `Ok(None)`.
pub async fn create(
    pool: &PgPool,
    user_id: &str,
    new: NewMatrix,
) -> AppResult<Option<BehaviorMatrix>> {
    let id = crate::new_id();
    let row = match new.space_id.as_deref() {
        Some(space_id) => {
            sqlx::query_as!(
                BehaviorMatrix,
                r#"INSERT INTO behavior_matrix (id, name, layer, description, space_id, user_id)
                   SELECT $1, $2, $3, $4, $5, $6
                   WHERE EXISTS (SELECT 1 FROM space s WHERE s.id = $5 AND s.user_id = $6)
                   RETURNING id, name, layer, description, space_id, user_id,
                             created_at AS "created_at: UtcTimestamp",
                             updated_at AS "updated_at: UtcTimestamp""#,
                id,
                new.name,
                new.layer,
                new.description,
                space_id,
                user_id
            )
            .fetch_optional(pool)
            .await?
        }
        None => Some(
            sqlx::query_as!(
                BehaviorMatrix,
                r#"INSERT INTO behavior_matrix (id, name, layer, description, space_id, user_id)
                   VALUES ($1, $2, $3, $4, NULL, $5)
                   RETURNING id, name, layer, description, space_id, user_id,
                             created_at AS "created_at: UtcTimestamp",
                             updated_at AS "updated_at: UtcTimestamp""#,
                id,
                new.name,
                new.layer,
                new.description,
                user_id
            )
            .fetch_one(pool)
            .await?,
        ),
    };
    Ok(row)
}

pub async fn find_by_id(
    pool: &PgPool,
    id: &str,
    user_id: &str,
) -> AppResult<Option<BehaviorMatrix>> {
    let row = sqlx::query_as!(
        BehaviorMatrix,
        r#"SELECT id, name, layer, description, space_id, user_id,
                  created_at AS "created_at: UtcTimestamp",
                  updated_at AS "updated_at: UtcTimestamp"
           FROM behavior_matrix WHERE id = $1 AND user_id = $2"#,
        id,
        user_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// `ORDER BY name, id` — the `, id` tiebreaker is this port's addition;
/// Node orders by name alone, which is a query-plan artifact over matrices
/// sharing a name (nothing stops two). Same rationale as `chunk::list`.
pub async fn list(
    pool: &PgPool,
    user_id: &str,
    space_id: Option<&str>,
    layer: Option<&str>,
) -> AppResult<Vec<BehaviorMatrix>> {
    let rows = sqlx::query_as!(
        BehaviorMatrix,
        r#"SELECT id, name, layer, description, space_id, user_id,
                  created_at AS "created_at: UtcTimestamp",
                  updated_at AS "updated_at: UtcTimestamp"
           FROM behavior_matrix
           WHERE user_id = $1
             AND ($2::text IS NULL OR space_id = $2)
             AND ($3::text IS NULL OR layer = $3)
           ORDER BY name, id"#,
        user_id,
        space_id,
        layer
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// `description` is tri-state (Node's body types it `string | null`), so it
/// takes the same `Option<Option<String>>` shape as `chunk`'s `summary`:
/// `None` leaves the column, `Some(None)` clears it.
#[derive(Default)]
pub struct MatrixPatch {
    pub name: Option<String>,
    pub description: Option<Option<String>>,
}

pub async fn update(
    pool: &PgPool,
    id: &str,
    user_id: &str,
    patch: MatrixPatch,
) -> AppResult<Option<BehaviorMatrix>> {
    let row = sqlx::query_as!(
        BehaviorMatrix,
        r#"UPDATE behavior_matrix SET
             name = COALESCE($3, name),
             description = CASE WHEN $4 THEN $5 ELSE description END,
             updated_at = now()
           WHERE id = $1 AND user_id = $2
           RETURNING id, name, layer, description, space_id, user_id,
                     created_at AS "created_at: UtcTimestamp",
                     updated_at AS "updated_at: UtcTimestamp""#,
        id,
        user_id,
        patch.name,
        patch.description.is_some(),
        patch.description.flatten()
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Everything below the matrix cascades off it — dimensions, rules, and
/// transitively cells, requirement links, code links, test results and rule
/// versions.
pub async fn delete(pool: &PgPool, id: &str, user_id: &str) -> AppResult<Option<BehaviorMatrix>> {
    let row = sqlx::query_as!(
        BehaviorMatrix,
        r#"DELETE FROM behavior_matrix WHERE id = $1 AND user_id = $2
           RETURNING id, name, layer, description, space_id, user_id,
                     created_at AS "created_at: UtcTimestamp",
                     updated_at AS "updated_at: UtcTimestamp""#,
        id,
        user_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

// ---------------------------------------------------------------------------
// Dimensions
// ---------------------------------------------------------------------------
//
// Every function is scoped through the matrix's owner in SQL. Node's
// equivalents take `(id, matrixId)` and rely on the service having checked
// the matrix — true for dimensions, but not a guarantee the repository
// itself provides.

/// Inserts a dimension at `max(order) + 1` within the matrix, computed in the
/// INSERT rather than by a prior SELECT so two concurrent adds cannot both
/// read the same max. `behavior_dimension_matrix_name` (UNIQUE on
/// `matrix_id, name`) rejects a duplicate name, surfacing as a 409.
///
/// Returns `Ok(None)` when the matrix is not the caller's — the
/// `WHERE EXISTS` matches nothing and the INSERT inserts nothing.
pub async fn create_dimension(
    pool: &PgPool,
    matrix_id: &str,
    user_id: &str,
    name: &str,
) -> AppResult<Option<BehaviorDimension>> {
    let id = crate::new_id();
    let row = sqlx::query_as!(
        BehaviorDimension,
        r#"INSERT INTO behavior_dimension (id, matrix_id, name, "order")
           SELECT $1, $2, $3,
                  COALESCE((SELECT MAX(d."order") FROM behavior_dimension d
                            WHERE d.matrix_id = $2), -1) + 1
           WHERE EXISTS (SELECT 1 FROM behavior_matrix m
                         WHERE m.id = $2 AND m.user_id = $4)
           RETURNING id, matrix_id, name, "order",
                     created_at AS "created_at: UtcTimestamp""#,
        id,
        matrix_id,
        name,
        user_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn update_dimension(
    pool: &PgPool,
    id: &str,
    matrix_id: &str,
    user_id: &str,
    name: &str,
) -> AppResult<Option<BehaviorDimension>> {
    let row = sqlx::query_as!(
        BehaviorDimension,
        r#"UPDATE behavior_dimension SET name = $4
           WHERE id = $1 AND matrix_id = $2
             AND EXISTS (SELECT 1 FROM behavior_matrix m
                         WHERE m.id = $2 AND m.user_id = $3)
           RETURNING id, matrix_id, name, "order",
                     created_at AS "created_at: UtcTimestamp""#,
        id,
        matrix_id,
        user_id,
        name
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn delete_dimension(
    pool: &PgPool,
    id: &str,
    matrix_id: &str,
    user_id: &str,
) -> AppResult<Option<BehaviorDimension>> {
    let row = sqlx::query_as!(
        BehaviorDimension,
        r#"DELETE FROM behavior_dimension
           WHERE id = $1 AND matrix_id = $2
             AND EXISTS (SELECT 1 FROM behavior_matrix m
                         WHERE m.id = $2 AND m.user_id = $3)
           RETURNING id, matrix_id, name, "order",
                     created_at AS "created_at: UtcTimestamp""#,
        id,
        matrix_id,
        user_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// `ORDER BY "order", id` — the `, id` tiebreaker is this port's addition
/// (Node orders by `order` alone). Two dimensions can share an order: the
/// column has no unique constraint, and `reorder_dimensions` renumbers only
/// the ids it is given, so a partial list leaves duplicates behind.
pub async fn dimensions_for_matrix(
    pool: &PgPool,
    matrix_id: &str,
    user_id: &str,
) -> AppResult<Vec<BehaviorDimension>> {
    let rows = sqlx::query_as!(
        BehaviorDimension,
        r#"SELECT id, matrix_id, name, "order",
                  created_at AS "created_at: UtcTimestamp"
           FROM behavior_dimension
           WHERE matrix_id = $1
             AND EXISTS (SELECT 1 FROM behavior_matrix m
                         WHERE m.id = $1 AND m.user_id = $2)
           ORDER BY "order", id"#,
        matrix_id,
        user_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Renumbers the given dimensions to their position in `dimension_ids`.
///
/// **Diverges from Node deliberately.** Node's `reorderDimensions(ids)` takes
/// a bare id list and updates whatever it names — the service checks that the
/// *matrix* belongs to the caller but never that the *ids* belong to that
/// matrix, so a caller owning any matrix can renumber another's columns. Here
/// `matrix_id` is in the UPDATE's WHERE clause, so ids from elsewhere are
/// silently skipped rather than reordered.
///
/// Done as one statement over `unnest` rather than a loop of UPDATEs (Node
/// issues one per id), so the renumbering is atomic: a failure part-way
/// cannot leave half the columns in the new order and half in the old.
pub async fn reorder_dimensions(
    pool: &PgPool,
    matrix_id: &str,
    user_id: &str,
    dimension_ids: &[String],
) -> AppResult<u64> {
    if dimension_ids.is_empty() {
        return Ok(0);
    }
    let affected = sqlx::query!(
        r#"UPDATE behavior_dimension d
           SET "order" = v.idx
           FROM (SELECT id, (ordinality - 1)::int AS idx
                 FROM unnest($3::text[]) WITH ORDINALITY AS t(id, ordinality)) AS v
           WHERE d.id = v.id
             AND d.matrix_id = $1
             AND EXISTS (SELECT 1 FROM behavior_matrix m
                         WHERE m.id = $1 AND m.user_id = $2)"#,
        matrix_id,
        user_id,
        dimension_ids
    )
    .execute(pool)
    .await?
    .rows_affected();
    Ok(affected)
}

// ---------------------------------------------------------------------------
// Rules
// ---------------------------------------------------------------------------

pub struct NewRule {
    pub title: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub rationale: Option<String>,
    pub alternatives: Option<String>,
    pub consequences: Option<String>,
    pub counterexample: Option<String>,
}

/// Same `max(order) + 1`-inside-the-INSERT shape as `create_dimension`, and
/// the same `Ok(None)` on a matrix the caller does not own. Unlike
/// dimensions, rule titles are NOT unique per matrix — there is no
/// constraint, and two rules may share a title.
pub async fn create_rule(
    pool: &PgPool,
    matrix_id: &str,
    user_id: &str,
    new: NewRule,
) -> AppResult<Option<BehaviorRule>> {
    let id = crate::new_id();
    let row = sqlx::query_as!(
        BehaviorRule,
        r#"INSERT INTO behavior_rule
             (id, matrix_id, title, description, category,
              rationale, alternatives, consequences, counterexample, "order")
           SELECT $1, $2, $3, $4, $5, $6, $7, $8, $9,
                  COALESCE((SELECT MAX(r."order") FROM behavior_rule r
                            WHERE r.matrix_id = $2), -1) + 1
           WHERE EXISTS (SELECT 1 FROM behavior_matrix m
                         WHERE m.id = $2 AND m.user_id = $10)
           RETURNING id, matrix_id, title, description, category,
                     rationale, alternatives, consequences, counterexample,
                     "order",
                     created_at AS "created_at: UtcTimestamp",
                     updated_at AS "updated_at: UtcTimestamp""#,
        id,
        matrix_id,
        new.title,
        new.description,
        new.category,
        new.rationale,
        new.alternatives,
        new.consequences,
        new.counterexample,
        user_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn find_rule(
    pool: &PgPool,
    id: &str,
    matrix_id: &str,
    user_id: &str,
) -> AppResult<Option<BehaviorRule>> {
    let row = sqlx::query_as!(
        BehaviorRule,
        r#"SELECT id, matrix_id, title, description, category,
                  rationale, alternatives, consequences, counterexample, "order",
                  created_at AS "created_at: UtcTimestamp",
                  updated_at AS "updated_at: UtcTimestamp"
           FROM behavior_rule
           WHERE id = $1 AND matrix_id = $2
             AND EXISTS (SELECT 1 FROM behavior_matrix m
                         WHERE m.id = $2 AND m.user_id = $3)"#,
        id,
        matrix_id,
        user_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Every field except `title` is tri-state — Node's PATCH body types all six
/// as `T | null`, and its repo spreads on `!== undefined`, so an explicit
/// `null` clears while an absent key leaves the column alone. `title` has no
/// null variant in Node's schema, so it stays two-state.
#[derive(Default)]
pub struct RulePatch {
    pub title: Option<String>,
    pub description: Option<Option<String>>,
    pub category: Option<Option<String>>,
    pub rationale: Option<Option<String>>,
    pub alternatives: Option<Option<String>>,
    pub consequences: Option<Option<String>>,
    pub counterexample: Option<Option<String>>,
}

pub async fn update_rule(
    pool: &PgPool,
    id: &str,
    matrix_id: &str,
    user_id: &str,
    patch: RulePatch,
) -> AppResult<Option<BehaviorRule>> {
    let row = sqlx::query_as!(
        BehaviorRule,
        r#"UPDATE behavior_rule SET
             title          = COALESCE($4, title),
             description    = CASE WHEN $5  THEN $6  ELSE description    END,
             category       = CASE WHEN $7  THEN $8  ELSE category       END,
             rationale      = CASE WHEN $9  THEN $10 ELSE rationale      END,
             alternatives   = CASE WHEN $11 THEN $12 ELSE alternatives   END,
             consequences   = CASE WHEN $13 THEN $14 ELSE consequences   END,
             counterexample = CASE WHEN $15 THEN $16 ELSE counterexample END,
             updated_at = now()
           WHERE id = $1 AND matrix_id = $2
             AND EXISTS (SELECT 1 FROM behavior_matrix m
                         WHERE m.id = $2 AND m.user_id = $3)
           RETURNING id, matrix_id, title, description, category,
                     rationale, alternatives, consequences, counterexample,
                     "order",
                     created_at AS "created_at: UtcTimestamp",
                     updated_at AS "updated_at: UtcTimestamp""#,
        id,
        matrix_id,
        user_id,
        patch.title,
        patch.description.is_some(),
        patch.description.flatten(),
        patch.category.is_some(),
        patch.category.flatten(),
        patch.rationale.is_some(),
        patch.rationale.flatten(),
        patch.alternatives.is_some(),
        patch.alternatives.flatten(),
        patch.consequences.is_some(),
        patch.consequences.flatten(),
        patch.counterexample.is_some(),
        patch.counterexample.flatten()
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn delete_rule(
    pool: &PgPool,
    id: &str,
    matrix_id: &str,
    user_id: &str,
) -> AppResult<Option<BehaviorRule>> {
    let row = sqlx::query_as!(
        BehaviorRule,
        r#"DELETE FROM behavior_rule
           WHERE id = $1 AND matrix_id = $2
             AND EXISTS (SELECT 1 FROM behavior_matrix m
                         WHERE m.id = $2 AND m.user_id = $3)
           RETURNING id, matrix_id, title, description, category,
                     rationale, alternatives, consequences, counterexample,
                     "order",
                     created_at AS "created_at: UtcTimestamp",
                     updated_at AS "updated_at: UtcTimestamp""#,
        id,
        matrix_id,
        user_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// `, id` tiebreaker — see `dimensions_for_matrix`.
pub async fn rules_for_matrix(
    pool: &PgPool,
    matrix_id: &str,
    user_id: &str,
) -> AppResult<Vec<BehaviorRule>> {
    let rows = sqlx::query_as!(
        BehaviorRule,
        r#"SELECT id, matrix_id, title, description, category,
                  rationale, alternatives, consequences, counterexample, "order",
                  created_at AS "created_at: UtcTimestamp",
                  updated_at AS "updated_at: UtcTimestamp"
           FROM behavior_rule
           WHERE matrix_id = $1
             AND EXISTS (SELECT 1 FROM behavior_matrix m
                         WHERE m.id = $1 AND m.user_id = $2)
           ORDER BY "order", id"#,
        matrix_id,
        user_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// See `reorder_dimensions` — same divergence, same atomicity argument.
pub async fn reorder_rules(
    pool: &PgPool,
    matrix_id: &str,
    user_id: &str,
    rule_ids: &[String],
) -> AppResult<u64> {
    if rule_ids.is_empty() {
        return Ok(0);
    }
    let affected = sqlx::query!(
        r#"UPDATE behavior_rule r
           SET "order" = v.idx
           FROM (SELECT id, (ordinality - 1)::int AS idx
                 FROM unnest($3::text[]) WITH ORDINALITY AS t(id, ordinality)) AS v
           WHERE r.id = v.id
             AND r.matrix_id = $1
             AND EXISTS (SELECT 1 FROM behavior_matrix m
                         WHERE m.id = $1 AND m.user_id = $2)"#,
        matrix_id,
        user_id,
        rule_ids
    )
    .execute(pool)
    .await?
    .rows_affected();
    Ok(affected)
}

/// Appends the rule's pre-edit state to its history. Scoped three hops
/// (version -> rule -> matrix -> user) so a snapshot cannot be written
/// against another user's rule.
pub async fn insert_rule_version(
    pool: &PgPool,
    rule_id: &str,
    matrix_id: &str,
    user_id: &str,
    snapshot: &BehaviorRuleSnapshot,
) -> AppResult<Option<BehaviorRuleVersion>> {
    let id = crate::new_id();
    let snapshot = serde_json::to_value(snapshot).map_err(|e| {
        fubbik_core::error::AppError::Validation(format!("failed to serialise rule snapshot: {e}"))
    })?;
    let row = sqlx::query_as!(
        BehaviorRuleVersion,
        r#"INSERT INTO behavior_rule_version (id, rule_id, snapshot, changed_by)
           SELECT $1, $2, $3, $4
           WHERE EXISTS (SELECT 1 FROM behavior_rule r
                         JOIN behavior_matrix m ON m.id = r.matrix_id
                         WHERE r.id = $2 AND r.matrix_id = $5 AND m.user_id = $4)
           RETURNING id, rule_id,
                     snapshot AS "snapshot: Json<BehaviorRuleSnapshot>",
                     changed_by,
                     created_at AS "created_at: UtcTimestamp""#,
        id,
        rule_id,
        snapshot,
        user_id,
        matrix_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Newest first. Node has **no `ORDER BY`** on this query at all, which for a
/// history list is a query-plan artifact rather than a choice — the UI
/// renders it as a timeline. `created_at DESC, id DESC` is this port's
/// addition; the `id` tiebreaker matters because two snapshots of the same
/// rule can land in the same millisecond.
pub async fn rule_versions(
    pool: &PgPool,
    rule_id: &str,
    matrix_id: &str,
    user_id: &str,
) -> AppResult<Vec<BehaviorRuleVersion>> {
    let rows = sqlx::query_as!(
        BehaviorRuleVersion,
        r#"SELECT v.id, v.rule_id,
                  v.snapshot AS "snapshot: Json<BehaviorRuleSnapshot>",
                  v.changed_by,
                  v.created_at AS "created_at: UtcTimestamp"
           FROM behavior_rule_version v
           JOIN behavior_rule r ON r.id = v.rule_id
           JOIN behavior_matrix m ON m.id = r.matrix_id
           WHERE v.rule_id = $1 AND r.matrix_id = $2 AND m.user_id = $3
           ORDER BY v.created_at DESC, v.id DESC"#,
        rule_id,
        matrix_id,
        user_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

// ---------------------------------------------------------------------------
// Cells
// ---------------------------------------------------------------------------
//
// This is the surface Node shipped with no authorization at all — nine
// routes, six of them writes, reachable by any authenticated user against
// any matrix. Every function below carries the ownership chain in SQL.

/// The cell at a `(rule, dimension)` intersection, if it exists AND both ends
/// live in a matrix owned by `user_id`.
pub async fn find_cell(
    pool: &PgPool,
    rule_id: &str,
    dimension_id: &str,
    matrix_id: &str,
    user_id: &str,
) -> AppResult<Option<BehaviorCell>> {
    let row = sqlx::query_as!(
        BehaviorCell,
        r#"SELECT c.id, c.rule_id, c.dimension_id,
                  c.created_at AS "created_at: UtcTimestamp"
           FROM behavior_cell c
           JOIN behavior_rule r ON r.id = c.rule_id
           JOIN behavior_dimension d ON d.id = c.dimension_id
           JOIN behavior_matrix m ON m.id = r.matrix_id
           WHERE c.rule_id = $1 AND c.dimension_id = $2
             AND r.matrix_id = $3 AND d.matrix_id = $3 AND m.user_id = $4"#,
        rule_id,
        dimension_id,
        matrix_id,
        user_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// A cell by id, but only if it belongs to `matrix_id` and that matrix to
/// `user_id`. This is what every `/cells/{cellId}/...` sub-resource gates on.
///
/// The `matrix_id` hop is load-bearing on its own: without it, a caller who
/// owns *any* matrix could pass a `cellId` from someone else's.
pub async fn find_cell_by_id(
    pool: &PgPool,
    cell_id: &str,
    matrix_id: &str,
    user_id: &str,
) -> AppResult<Option<BehaviorCell>> {
    let row = sqlx::query_as!(
        BehaviorCell,
        r#"SELECT c.id, c.rule_id, c.dimension_id,
                  c.created_at AS "created_at: UtcTimestamp"
           FROM behavior_cell c
           JOIN behavior_rule r ON r.id = c.rule_id
           JOIN behavior_matrix m ON m.id = r.matrix_id
           WHERE c.id = $1 AND r.matrix_id = $2 AND m.user_id = $3"#,
        cell_id,
        matrix_id,
        user_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Creates the cell only if the rule AND the dimension both belong to
/// `matrix_id`, and the matrix to `user_id`. Pairing your own rule with a
/// stranger's dimension is rejected here, not just at the service layer.
///
/// `behavior_cell_rule_dimension` (UNIQUE on `rule_id, dimension_id`) makes a
/// double-toggle a unique violation rather than a duplicate row.
pub async fn create_cell(
    pool: &PgPool,
    rule_id: &str,
    dimension_id: &str,
    matrix_id: &str,
    user_id: &str,
) -> AppResult<Option<BehaviorCell>> {
    let id = crate::new_id();
    let row = sqlx::query_as!(
        BehaviorCell,
        r#"INSERT INTO behavior_cell (id, rule_id, dimension_id)
           SELECT $1, $2, $3
           WHERE EXISTS (SELECT 1 FROM behavior_rule r
                         JOIN behavior_matrix m ON m.id = r.matrix_id
                         WHERE r.id = $2 AND r.matrix_id = $4 AND m.user_id = $5)
             AND EXISTS (SELECT 1 FROM behavior_dimension d
                         WHERE d.id = $3 AND d.matrix_id = $4)
           RETURNING id, rule_id, dimension_id,
                     created_at AS "created_at: UtcTimestamp""#,
        id,
        rule_id,
        dimension_id,
        matrix_id,
        user_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn delete_cell(
    pool: &PgPool,
    cell_id: &str,
    matrix_id: &str,
    user_id: &str,
) -> AppResult<Option<BehaviorCell>> {
    let row = sqlx::query_as!(
        BehaviorCell,
        r#"DELETE FROM behavior_cell c
           USING behavior_rule r, behavior_matrix m
           WHERE c.id = $1
             AND r.id = c.rule_id AND r.matrix_id = $2
             AND m.id = r.matrix_id AND m.user_id = $3
           RETURNING c.id, c.rule_id, c.dimension_id,
                     c.created_at AS "created_at: UtcTimestamp""#,
        cell_id,
        matrix_id,
        user_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// How many requirements a cell has linked. Node refuses to delete a cell
/// with any, so this decides whether a toggle-off is allowed.
pub async fn cell_requirement_count(
    pool: &PgPool,
    cell_id: &str,
    matrix_id: &str,
    user_id: &str,
) -> AppResult<i64> {
    let count = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "count!"
           FROM behavior_cell_requirement cr
           JOIN behavior_cell c ON c.id = cr.cell_id
           JOIN behavior_rule r ON r.id = c.rule_id
           JOIN behavior_matrix m ON m.id = r.matrix_id
           WHERE cr.cell_id = $1 AND r.matrix_id = $2 AND m.user_id = $3"#,
        cell_id,
        matrix_id,
        user_id
    )
    .fetch_one(pool)
    .await?;
    Ok(count)
}

// ---------------------------------------------------------------------------
// Cell evidence: requirements, code links, test results
// ---------------------------------------------------------------------------

/// Links a requirement to a cell. **Both** ends are checked: the cell through
/// the matrix, and the requirement through its own `user_id`. Node checks
/// neither, so it allowed attaching your requirement to a stranger's cell —
/// and, in the other direction, attaching a stranger's requirement to yours.
pub async fn link_cell_requirement(
    pool: &PgPool,
    cell_id: &str,
    requirement_id: &str,
    matrix_id: &str,
    user_id: &str,
) -> AppResult<Option<CellRequirementLink>> {
    let row = sqlx::query_as!(
        CellRequirementLink,
        r#"INSERT INTO behavior_cell_requirement (cell_id, requirement_id)
           SELECT $1, $2
           WHERE EXISTS (SELECT 1 FROM behavior_cell c
                         JOIN behavior_rule r ON r.id = c.rule_id
                         JOIN behavior_matrix m ON m.id = r.matrix_id
                         WHERE c.id = $1 AND r.matrix_id = $3 AND m.user_id = $4)
             AND EXISTS (SELECT 1 FROM requirement q
                         WHERE q.id = $2 AND q.user_id = $4)
           ON CONFLICT (cell_id, requirement_id) DO NOTHING
           RETURNING cell_id, requirement_id"#,
        cell_id,
        requirement_id,
        matrix_id,
        user_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn unlink_cell_requirement(
    pool: &PgPool,
    cell_id: &str,
    requirement_id: &str,
    matrix_id: &str,
    user_id: &str,
) -> AppResult<Option<CellRequirementLink>> {
    let row = sqlx::query_as!(
        CellRequirementLink,
        r#"DELETE FROM behavior_cell_requirement cr
           USING behavior_cell c, behavior_rule r, behavior_matrix m
           WHERE cr.cell_id = $1 AND cr.requirement_id = $2
             AND c.id = cr.cell_id
             AND r.id = c.rule_id AND r.matrix_id = $3
             AND m.id = r.matrix_id AND m.user_id = $4
           RETURNING cr.cell_id, cr.requirement_id"#,
        cell_id,
        requirement_id,
        matrix_id,
        user_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// **No `ORDER BY`**, matching Node exactly.
pub async fn requirements_for_cell(
    pool: &PgPool,
    cell_id: &str,
    matrix_id: &str,
    user_id: &str,
) -> AppResult<Vec<CellRequirement>> {
    let rows = sqlx::query_as!(
        CellRequirement,
        r#"SELECT cr.requirement_id, q.title, q.status
           FROM behavior_cell_requirement cr
           JOIN requirement q ON q.id = cr.requirement_id
           JOIN behavior_cell c ON c.id = cr.cell_id
           JOIN behavior_rule r ON r.id = c.rule_id
           JOIN behavior_matrix m ON m.id = r.matrix_id
           WHERE cr.cell_id = $1 AND r.matrix_id = $2 AND m.user_id = $3"#,
        cell_id,
        matrix_id,
        user_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn link_cell_code(
    pool: &PgPool,
    cell_id: &str,
    kind: &str,
    code_ref: &str,
    matrix_id: &str,
    user_id: &str,
) -> AppResult<Option<BehaviorCellCode>> {
    let id = crate::new_id();
    let row = sqlx::query_as!(
        BehaviorCellCode,
        r#"INSERT INTO behavior_cell_code (id, cell_id, kind, ref)
           SELECT $1, $2, $3, $4
           WHERE EXISTS (SELECT 1 FROM behavior_cell c
                         JOIN behavior_rule r ON r.id = c.rule_id
                         JOIN behavior_matrix m ON m.id = r.matrix_id
                         WHERE c.id = $2 AND r.matrix_id = $5 AND m.user_id = $6)
           ON CONFLICT (cell_id, kind, ref) DO NOTHING
           RETURNING id, cell_id, kind, ref AS code_ref,
                     created_at AS "created_at: UtcTimestamp""#,
        id,
        cell_id,
        kind,
        code_ref,
        matrix_id,
        user_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn delete_cell_code(
    pool: &PgPool,
    code_id: &str,
    cell_id: &str,
    matrix_id: &str,
    user_id: &str,
) -> AppResult<Option<BehaviorCellCode>> {
    let row = sqlx::query_as!(
        BehaviorCellCode,
        r#"DELETE FROM behavior_cell_code cc
           USING behavior_cell c, behavior_rule r, behavior_matrix m
           WHERE cc.id = $1 AND cc.cell_id = $2
             AND c.id = cc.cell_id
             AND r.id = c.rule_id AND r.matrix_id = $3
             AND m.id = r.matrix_id AND m.user_id = $4
           RETURNING cc.id, cc.cell_id, cc.kind, cc.ref AS code_ref,
                     cc.created_at AS "created_at: UtcTimestamp""#,
        code_id,
        cell_id,
        matrix_id,
        user_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// **No `ORDER BY`**, matching Node.
pub async fn code_for_cell(
    pool: &PgPool,
    cell_id: &str,
    matrix_id: &str,
    user_id: &str,
) -> AppResult<Vec<BehaviorCellCode>> {
    let rows = sqlx::query_as!(
        BehaviorCellCode,
        r#"SELECT cc.id, cc.cell_id, cc.kind, cc.ref AS code_ref,
                  cc.created_at AS "created_at: UtcTimestamp"
           FROM behavior_cell_code cc
           JOIN behavior_cell c ON c.id = cc.cell_id
           JOIN behavior_rule r ON r.id = c.rule_id
           JOIN behavior_matrix m ON m.id = r.matrix_id
           WHERE cc.cell_id = $1 AND r.matrix_id = $2 AND m.user_id = $3"#,
        cell_id,
        matrix_id,
        user_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Reverse lookup: which behaviours govern a file.
///
/// The three-way match is Node's verbatim
/// (`packages/db/src/repository/behavior-matrix.ts:404`): an exact ref, a ref
/// that is a suffix of the queried path (so `src/a/b.ts` matches a link
/// stored as `b.ts`), or a `path::symbol` link on that path. Already scoped
/// by `user_id` in Node — one of the few matrix queries that was.
pub async fn behaviors_for_path(
    pool: &PgPool,
    user_id: &str,
    path: &str,
) -> AppResult<Vec<BehaviorForFile>> {
    let rows = sqlx::query_as!(
        BehaviorForFile,
        r#"SELECT r.id AS rule_id, r.title AS rule_title, r.description,
                  r.rationale, r.counterexample,
                  m.id AS matrix_id, m.name AS matrix_name, m.layer,
                  d.name AS dimension_name,
                  cc.kind, cc.ref AS code_ref
           FROM behavior_cell_code cc
           JOIN behavior_cell c ON c.id = cc.cell_id
           JOIN behavior_rule r ON r.id = c.rule_id
           JOIN behavior_dimension d ON d.id = c.dimension_id
           JOIN behavior_matrix m ON m.id = r.matrix_id
           WHERE m.user_id = $1
             AND (cc.ref = $2 OR $2 LIKE '%' || cc.ref OR cc.ref LIKE $2 || '::%')"#,
        user_id,
        path
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn record_test_result(
    pool: &PgPool,
    cell_id: &str,
    test_ref: &str,
    status: &str,
    detail: Option<&str>,
    matrix_id: &str,
    user_id: &str,
) -> AppResult<Option<BehaviorTestResult>> {
    let id = crate::new_id();
    let row = sqlx::query_as!(
        BehaviorTestResult,
        r#"INSERT INTO behavior_test_result (id, cell_id, test_ref, status, detail)
           SELECT $1, $2, $3, $4, $5
           WHERE EXISTS (SELECT 1 FROM behavior_cell c
                         JOIN behavior_rule r ON r.id = c.rule_id
                         JOIN behavior_matrix m ON m.id = r.matrix_id
                         WHERE c.id = $2 AND r.matrix_id = $6 AND m.user_id = $7)
           RETURNING id, cell_id, test_ref, status, detail,
                     run_at AS "run_at: UtcTimestamp""#,
        id,
        cell_id,
        test_ref,
        status,
        detail,
        matrix_id,
        user_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Newest run first. Node has no `ORDER BY`; `run_at DESC, id DESC` is this
/// port's addition, for the same reason as `rule_versions` — this is a log,
/// and two runs of the same test can share a timestamp.
pub async fn test_results_for_cell(
    pool: &PgPool,
    cell_id: &str,
    matrix_id: &str,
    user_id: &str,
) -> AppResult<Vec<BehaviorTestResult>> {
    let rows = sqlx::query_as!(
        BehaviorTestResult,
        r#"SELECT tr.id, tr.cell_id, tr.test_ref, tr.status, tr.detail,
                  tr.run_at AS "run_at: UtcTimestamp"
           FROM behavior_test_result tr
           JOIN behavior_cell c ON c.id = tr.cell_id
           JOIN behavior_rule r ON r.id = c.rule_id
           JOIN behavior_matrix m ON m.id = r.matrix_id
           WHERE tr.cell_id = $1 AND r.matrix_id = $2 AND m.user_id = $3
           ORDER BY tr.run_at DESC, tr.id DESC"#,
        cell_id,
        matrix_id,
        user_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Per-cell aggregates for the computed matrix view.
///
/// Node runs three queries and stitches them in JS, specifically to avoid
/// join fan-out (counting requirements and code links in one query would
/// multiply both). This is one query with three scalar subqueries, which
/// avoids the fan-out the same way and additionally makes the counts a
/// consistent snapshot — Node's three reads can interleave with a concurrent
/// write and report a cell as having a code link but no requirement that was
/// there a moment earlier.
pub async fn matrix_view_cells(
    pool: &PgPool,
    matrix_id: &str,
    user_id: &str,
) -> AppResult<Vec<MatrixViewCell>> {
    let rows = sqlx::query_as!(
        MatrixViewCell,
        r#"SELECT c.id AS "id!", c.rule_id AS "rule_id!", c.dimension_id AS "dimension_id!",
                  (SELECT COUNT(*) FROM behavior_cell_requirement cr
                   WHERE cr.cell_id = c.id) AS "requirement_count!",
                  (SELECT COUNT(*) FROM behavior_cell_requirement cr
                   JOIN requirement q ON q.id = cr.requirement_id
                   WHERE cr.cell_id = c.id AND q.status = 'failing') AS "failing_count!",
                  (SELECT COUNT(*) FROM behavior_cell_code cc
                   WHERE cc.cell_id = c.id) AS "code_count!",
                  (SELECT COUNT(*) FROM behavior_test_result tr
                   WHERE tr.cell_id = c.id AND tr.status = 'pass') AS "passing_test_count!",
                  (SELECT COUNT(*) FROM behavior_test_result tr
                   WHERE tr.cell_id = c.id AND tr.status = 'fail') AS "failing_test_count!"
           FROM behavior_cell c
           JOIN behavior_rule r ON r.id = c.rule_id
           JOIN behavior_matrix m ON m.id = r.matrix_id
           WHERE r.matrix_id = $1 AND m.user_id = $2
           ORDER BY c.id"#,
        matrix_id,
        user_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}
