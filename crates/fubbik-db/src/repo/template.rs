//! Chunk templates (`chunk_template`) back `POST /api/chunks/new`'s template
//! picker and the match/extraction engine used by document imports. The
//! CRUD persistence lives here; matching and extraction live in
//! `fubbik_api::documents::template_import`.
//!
//! **Built-in templates are seeded by migration and read-only.** Node's
//! `updateTemplate`/`deleteTemplate` services
//! (`packages/api/src/templates/service.ts:46-79`) both load the row
//! unscoped via `getTemplateById`, reject with `ValidationError` when
//! `isBuiltIn` is true, and only then call the scoped repo mutation. That
//! service-layer check is replicated verbatim in
//! `fubbik_api::templates::service`. This module additionally carries the
//! two SQL-level guards Node itself has:
//!
//! - `update`/`delete` both filter `WHERE id = $1 AND user_id = $2`. Every
//!   built-in row has `user_id IS NULL`, so no real caller's `user_id` can
//!   ever equal it — cross-user protection and (incidentally) built-in
//!   protection for `update`, matching
//!   `packages/db/src/repository/template.ts:57-88` exactly (no explicit
//!   `is_built_in` check in Node's `UPDATE`).
//! - `delete` additionally filters `AND is_built_in = false` explicitly
//!   (`packages/db/src/repository/template.ts:90-98`) — belt-and-suspenders
//!   with the `user_id` guard above. See `tests/template.rs` for a
//!   constructed row (`is_built_in = true` with a real `user_id`, a state
//!   the app itself never produces) that isolates this guard from the
//!   `user_id` one and proves it's independently load-bearing.

use fubbik_core::error::AppResult;
use sqlx::PgPool;
use sqlx::types::Json;

use crate::timestamp::UtcTimestamp;

/// `"exact" | "prefix" | "contains"` — a real Elysia `t.Union` of literals
/// in Node (`packages/api/src/templates/routes.ts:7`), not free text, so
/// modelling it as an enum here matches the contract rather than adding a
/// constraint Node lacks.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, utoipa::ToSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum MatchMode {
    Exact,
    Prefix,
    Contains,
}

/// `"exact" | "oneOf" | "exists"` — likewise a real `t.Union` of literals
/// (`packages/api/src/templates/routes.ts:18`).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, utoipa::ToSchema,
)]
#[serde(rename_all = "camelCase")]
pub enum FrontmatterMatchMode {
    Exact,
    OneOf,
    Exists,
}

/// The six chunk fields a template's field mappings can populate — a real
/// `t.Union` of literals (`packages/api/src/templates/routes.ts:29-36`).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, utoipa::ToSchema,
)]
#[serde(rename_all = "camelCase")]
pub enum ExtractionTarget {
    Rationale,
    Alternatives,
    Consequences,
    Summary,
    Scope,
    Content,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct HeadingRule {
    pub patterns: Vec<String>,
    #[serde(rename = "match")]
    pub match_mode: MatchMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub level: Option<i32>,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FrontmatterRule {
    pub key: String,
    #[serde(rename = "match")]
    pub match_mode: FrontmatterMatchMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub values: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MatchRules {
    pub min_score: f64,
    pub headings: Vec<HeadingRule>,
    pub frontmatter: Vec<FrontmatterRule>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FieldMapping {
    pub headings: Vec<String>,
    #[serde(rename = "match")]
    pub match_mode: MatchMode,
    pub target: ExtractionTarget,
}

/// `camelCase` serialisation matches every other wire type in this crate.
/// `type`/`content` default to `"note"`/`""` at the column level
/// (`packages/db/src/schema/template.ts:34-35`) but Node's `createTemplate`
/// always supplies both explicitly, so those defaults are never actually
/// observed through the API — kept here only because the column itself
/// isn't `NOT NULL`-without-default.
///
/// `type` is `text NOT NULL` with only a `maxLength: 20` Elysia check
/// (`packages/api/src/templates/routes.ts:65`), not an enum or CHECK
/// constraint, despite the five chunk kinds (note/document/reference/
/// schema/checklist) listed in CLAUDE.md — matching the established
/// "constrained-looking free text" trap, this stays `String`.
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Template {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    #[serde(rename = "type")]
    pub template_type: String,
    pub content: String,
    pub is_built_in: bool,
    #[schema(value_type = Option<MatchRules>)]
    pub match_rules: Option<Json<MatchRules>>,
    #[schema(value_type = Option<Vec<FieldMapping>>)]
    pub field_mappings: Option<Json<Vec<FieldMapping>>>,
    pub priority: i32,
    pub tags: Option<Vec<String>>,
    pub user_id: Option<String>,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub created_at: UtcTimestamp,
}

pub struct NewTemplate {
    pub name: String,
    pub description: Option<String>,
    pub template_type: String,
    pub content: String,
    pub match_rules: Option<MatchRules>,
    pub field_mappings: Option<Vec<FieldMapping>>,
    pub priority: Option<i32>,
    pub tags: Option<Vec<String>>,
}

/// Lists built-in templates plus the caller's own, matching Node's
/// `listTemplates` (`packages/db/src/repository/template.ts:8-15`):
/// `WHERE is_built_in = true OR user_id = $1`. Node has no `ORDER BY` at
/// all here — this port adds `ORDER BY name ASC, id ASC` for a
/// deterministic total order, the same shape as `collection::list`'s
/// equivalent addition.
pub async fn list(pool: &PgPool, user_id: &str) -> AppResult<Vec<Template>> {
    let rows = sqlx::query_as!(
        Template,
        r#"SELECT id, name, description, type AS template_type, content, is_built_in,
                  match_rules AS "match_rules: Json<MatchRules>",
                  field_mappings AS "field_mappings: Json<Vec<FieldMapping>>",
                  priority, tags, user_id,
                  created_at AS "created_at: UtcTimestamp"
           FROM chunk_template
           WHERE is_built_in = true OR user_id = $1
           ORDER BY name ASC, id ASC"#,
        user_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Unscoped by design, matching Node's `getTemplateById`
/// (`packages/db/src/repository/template.ts:17-22`): `WHERE id = $1` only.
/// Used by the service layer purely to decide "exists?" / "built-in?"
/// before attempting a scoped mutation — no field from this lookup is
/// itself returned to the caller, so this can't leak another user's
/// template content. The actual ownership guard lives in `update`/`delete`
/// below.
pub async fn find_by_id(pool: &PgPool, id: &str) -> AppResult<Option<Template>> {
    let row = sqlx::query_as!(
        Template,
        r#"SELECT id, name, description, type AS template_type, content, is_built_in,
                  match_rules AS "match_rules: Json<MatchRules>",
                  field_mappings AS "field_mappings: Json<Vec<FieldMapping>>",
                  priority, tags, user_id,
                  created_at AS "created_at: UtcTimestamp"
           FROM chunk_template WHERE id = $1"#,
        id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Always creates a non-built-in, user-owned row — matching Node's
/// `createTemplate`, which hardcodes `isBuiltIn: false`
/// (`packages/db/src/repository/template.ts:45`). There is no path through
/// this API that can create a built-in template.
pub async fn create(pool: &PgPool, user_id: &str, new: NewTemplate) -> AppResult<Template> {
    let id = crate::new_id();
    let row = sqlx::query_as!(
        Template,
        r#"INSERT INTO chunk_template
             (id, name, description, type, content, is_built_in, match_rules, field_mappings, priority, tags, user_id)
           VALUES ($1, $2, $3, $4, $5, false, $6, $7, $8, $9, $10)
           RETURNING id, name, description, type AS template_type, content, is_built_in,
                     match_rules AS "match_rules: Json<MatchRules>",
                     field_mappings AS "field_mappings: Json<Vec<FieldMapping>>",
                     priority, tags, user_id,
                     created_at AS "created_at: UtcTimestamp""#,
        id,
        new.name,
        new.description,
        new.template_type,
        new.content,
        new.match_rules.map(Json) as _,
        new.field_mappings.map(Json) as _,
        new.priority.unwrap_or(0),
        new.tags.as_deref(),
        user_id
    )
    .fetch_one(pool)
    .await?;
    Ok(row)
}

/// Two-state `Option<T>` fields (`None` = leave untouched), matching
/// Node's `COALESCE`-free but conditional-spread `.set({...})`
/// (`packages/db/src/repository/template.ts:74-83`), which only assigns a
/// column when the caller's field is `!== undefined`. `description` has no
/// way to be explicitly cleared through this endpoint (Node's PATCH body
/// allows `t.Null()` on `description`, but the repo layer's
/// `params.description !== undefined` spread would in fact forward an
/// explicit `null` through — see `update`'s `description` handling below,
/// which mirrors that by treating `Some(None)` as "clear it").
#[derive(Default)]
pub struct TemplatePatch {
    pub name: Option<String>,
    /// `Some(None)` clears the column (Node forwards an explicit `null`
    /// through); `None` leaves it untouched.
    pub description: Option<Option<String>>,
    pub template_type: Option<String>,
    pub content: Option<String>,
    /// `Some(None)` clears the column; `None` leaves it untouched.
    pub match_rules: Option<Option<MatchRules>>,
    /// `Some(None)` clears the column; `None` leaves it untouched.
    pub field_mappings: Option<Option<Vec<FieldMapping>>>,
    pub priority: Option<i32>,
    pub tags: Option<Vec<String>>,
}

/// `WHERE id = $1 AND user_id = $2` — the SQL-level ownership guard. A
/// built-in row's `user_id` is always `NULL`, which can never equal a real
/// caller's `user_id`, so this also incidentally blocks any attempt to
/// update a built-in template even if the service-layer `is_built_in`
/// pre-check were removed. See `tests/template.rs::update_is_scoped_to_owner`
/// for the cross-user proof.
pub async fn update(
    pool: &PgPool,
    user_id: &str,
    id: &str,
    patch: TemplatePatch,
) -> AppResult<Option<Template>> {
    let row = sqlx::query_as!(
        Template,
        r#"UPDATE chunk_template SET
             name = COALESCE($3, name),
             description = CASE WHEN $4 THEN $5 ELSE description END,
             type = COALESCE($6, type),
             content = COALESCE($7, content),
             match_rules = CASE WHEN $8 THEN $9 ELSE match_rules END,
             field_mappings = CASE WHEN $10 THEN $11 ELSE field_mappings END,
             priority = COALESCE($12, priority),
             tags = COALESCE($13, tags)
           WHERE id = $1 AND user_id = $2
           RETURNING id, name, description, type AS template_type, content, is_built_in,
                     match_rules AS "match_rules: Json<MatchRules>",
                     field_mappings AS "field_mappings: Json<Vec<FieldMapping>>",
                     priority, tags, user_id,
                     created_at AS "created_at: UtcTimestamp""#,
        id,
        user_id,
        patch.name,
        patch.description.is_some(),
        patch.description.flatten(),
        patch.template_type,
        patch.content,
        patch.match_rules.is_some(),
        patch.match_rules.flatten().map(Json) as _,
        patch.field_mappings.is_some(),
        patch.field_mappings.flatten().map(Json) as _,
        patch.priority,
        patch.tags.as_deref()
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// `WHERE id = $1 AND user_id = $2 AND is_built_in = false` — matching
/// Node's `deleteTemplate` exactly
/// (`packages/db/src/repository/template.ts:90-98`). The explicit
/// `is_built_in = false` is redundant with the `user_id` guard for every
/// row the app itself can produce (built-ins always have `user_id IS
/// NULL`), but it's independently load-bearing against a row that has
/// somehow ended up with both `is_built_in = true` and a real `user_id` —
/// see `tests/template.rs::delete_excludes_built_in_even_when_user_id_matches`,
/// which constructs exactly that row to isolate this guard from the
/// `user_id` one.
pub async fn delete(pool: &PgPool, user_id: &str, id: &str) -> AppResult<bool> {
    let res = sqlx::query!(
        "DELETE FROM chunk_template WHERE id = $1 AND user_id = $2 AND is_built_in = false",
        id,
        user_id
    )
    .execute(pool)
    .await?;
    Ok(res.rows_affected() > 0)
}
