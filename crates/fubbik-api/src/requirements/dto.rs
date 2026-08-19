use fubbik_db::repo::requirement::{Requirement, RequirementChunk, RequirementStep};

use super::cross_ref::CrossRefWarning;
use crate::vocabulary::parser::{Position, WarningType};

/// Matches Node's `t.Union([t.Literal("human"), t.Literal("ai")])`,
/// constrained everywhere it appears: create/update bodies AND the list
/// query filter (`packages/api/src/requirements/routes.ts:111,138,215`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Origin {
    Human,
    Ai,
}

impl Origin {
    pub fn as_str(self) -> &'static str {
        match self {
            Origin::Human => "human",
            Origin::Ai => "ai",
        }
    }
}

/// Matches Node's `t.Union([t.Literal("draft"), t.Literal("reviewed"),
/// t.Literal("approved")])`, constrained on the update body and the list
/// query filter (`packages/api/src/requirements/routes.ts:112,216`). Never
/// accepted on create — a new requirement's `review_status` is always
/// computed by the service from `origin` (`"draft"` for `ai`, `"approved"`
/// otherwise), matching Node's `createRequirement`
/// (`packages/api/src/requirements/service.ts:117,129`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReviewStatus {
    Draft,
    Reviewed,
    Approved,
}

impl ReviewStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            ReviewStatus::Draft => "draft",
            ReviewStatus::Reviewed => "reviewed",
            ReviewStatus::Approved => "approved",
        }
    }
}

/// Matches Node's `PrioritySchema` (`t.Union([t.Literal("must"),
/// t.Literal("should"), t.Literal("could"), t.Literal("wont")])`),
/// constrained on create/update bodies. The list query's `priority` filter
/// is unconstrained free text (`t.Optional(t.String())`) — see
/// `ListRequirementsQuery::priority`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Priority {
    Must,
    Should,
    Could,
    Wont,
}

impl Priority {
    pub fn as_str(self) -> &'static str {
        match self {
            Priority::Must => "must",
            Priority::Should => "should",
            Priority::Could => "could",
            Priority::Wont => "wont",
        }
    }
}

/// Matches Node's `StatusSchema` (`t.Union([t.Literal("passing"),
/// t.Literal("failing"), t.Literal("untested")])`), constrained on `PATCH
/// /requirements/{id}/status` and the `set_status` bulk action. The list
/// query's `status` filter is unconstrained free text — see
/// `ListRequirementsQuery::status`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Passing,
    Failing,
    Untested,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Passing => "passing",
            Status::Failing => "failing",
            Status::Untested => "untested",
        }
    }
}

/// Matches Node's `FormatSchema` (`t.Union([t.Literal("gherkin"),
/// t.Literal("vitest"), t.Literal("markdown")])`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Format {
    Gherkin,
    Vitest,
    Markdown,
}

impl Format {
    pub fn as_str(self) -> &'static str {
        match self {
            Format::Gherkin => "gherkin",
            Format::Vitest => "vitest",
            Format::Markdown => "markdown",
        }
    }
}

fn deserialize_some<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    T: serde::Deserialize<'de>,
    D: serde::Deserializer<'de>,
{
    T::deserialize(deserializer).map(Some)
}

/// Query params of `GET /requirements`
/// (`packages/api/src/requirements/routes.ts:105-116`). `status`/`priority`
/// are unconstrained free text (unlike the same-named fields on
/// create/update bodies) — see `fubbik_db::repo::requirement::Requirement`'s
/// doc comment for why. `limit`/`offset` arrive as strings, matching
/// Node's `t.Optional(t.String())` (parsed with `Number(...)` in the
/// service layer, same not-a-real-number-type quirk as
/// `notifications::dto::ListNotificationsQuery`).
#[derive(serde::Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct ListRequirementsQuery {
    pub space_id: Option<String>,
    pub use_case_id: Option<String>,
    pub search: Option<String>,
    pub status: Option<String>,
    pub priority: Option<String>,
    pub origin: Option<Origin>,
    pub review_status: Option<ReviewStatus>,
    pub limit: Option<String>,
    pub offset: Option<String>,
}

#[derive(serde::Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct StatsQuery {
    pub space_id: Option<String>,
}

#[derive(serde::Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct ExportAllQuery {
    pub format: Format,
    pub space_id: Option<String>,
}

#[derive(serde::Deserialize, utoipa::IntoParams)]
pub struct ExportOneQuery {
    pub format: Format,
}

/// Body of `POST /requirements` (`packages/api/src/requirements/routes.ts:
/// 131-139`).
#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateRequirementBody {
    pub title: String,
    pub description: Option<String>,
    pub steps: Vec<RequirementStep>,
    pub priority: Option<Priority>,
    pub space_id: Option<String>,
    pub use_case_id: Option<String>,
    pub origin: Option<Origin>,
}

/// Body of `PATCH /requirements/{id}` (`packages/api/src/requirements/
/// routes.ts:208-217`). `description`/`priority`/`space_id`/`use_case_id`
/// are tri-state (`None` = untouched, `Some(None)` = clear, `Some(Some(v))`
/// = set), matching Node's `t.Optional(t.Union([T, t.Null()]))` for each.
/// `title`/`steps`/`origin`/`review_status` have no null variant in Node's
/// schema, so plain `Option<T>`.
#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateRequirementBody {
    pub title: Option<String>,
    #[serde(default, deserialize_with = "deserialize_some")]
    pub description: Option<Option<String>>,
    pub steps: Option<Vec<RequirementStep>>,
    #[serde(default, deserialize_with = "deserialize_some")]
    pub priority: Option<Option<Priority>>,
    #[serde(default, deserialize_with = "deserialize_some")]
    pub space_id: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_some")]
    pub use_case_id: Option<Option<String>>,
    pub origin: Option<Origin>,
    pub review_status: Option<ReviewStatus>,
}

#[derive(serde::Deserialize, utoipa::ToSchema)]
pub struct UpdateStatusBody {
    pub status: Status,
}

#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SetChunksBody {
    pub chunk_ids: Vec<String>,
}

/// Body of `PATCH /requirements/bulk` (`packages/api/src/requirements/
/// routes.ts:49-55`). `use_case_id` is tri-state
/// (`t.Optional(t.Union([t.String(), t.Null()]))`) — only meaningful for
/// `action: "set_use_case"`.
#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BulkActionBody {
    pub ids: Vec<String>,
    pub action: BulkActionKind,
    pub status: Option<Status>,
    #[serde(default, deserialize_with = "deserialize_some")]
    pub use_case_id: Option<Option<String>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum BulkActionKind {
    SetStatus,
    SetUseCase,
    Delete,
}

#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReorderBody {
    pub requirement_ids: Vec<String>,
}

/// One entry of `POST /requirements/batch`'s `requirements` array
/// (`packages/api/src/requirements/routes.ts:172-182`).
#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BatchRequirementInput {
    pub title: String,
    pub description: Option<String>,
    pub steps: Vec<RequirementStep>,
    pub priority: Option<Priority>,
    pub use_case_id: Option<String>,
    pub use_case_name: Option<String>,
    pub parent_use_case_name: Option<String>,
}

#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BatchCreateBody {
    pub requirements: Vec<BatchRequirementInput>,
    pub space_id: Option<String>,
}

/// Shape of `GET /requirements`: `{requirements, total}`, matching Node's
/// `listRequirementsRepo` return expression returned bare by the route
/// (`packages/api/src/requirements/service.ts:75-87`, `routes.ts:95-103`)
/// — not wrapped further, and no `limit`/`offset` echoed back (unlike
/// `chunks::dto::ListChunksResponse`).
#[derive(serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ListRequirementsResponse {
    pub requirements: Vec<Requirement>,
    pub total: i64,
}

/// Shape of `GET /requirements/{id}`, matching Node's `getRequirement`
/// return expression: `{ ...req, chunks }`
/// (`packages/api/src/requirements/service.ts:89-97`) — a genuine object
/// spread, so the requirement's own fields sit at the *top level* alongside
/// `chunks`. Modelled as an explicit flat struct, same convention
/// `documents::dto::DocumentDetail` documents (no DTO in this crate uses
/// `#[serde(flatten)]`, to keep the utoipa-generated schema unambiguous).
#[derive(serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RequirementDetail {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub steps: Vec<RequirementStep>,
    pub order: i32,
    pub status: String,
    pub priority: Option<String>,
    pub space_id: Option<String>,
    pub user_id: String,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub created_at: fubbik_db::timestamp::UtcTimestamp,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub updated_at: fubbik_db::timestamp::UtcTimestamp,
    pub origin: String,
    pub review_status: String,
    pub use_case_id: Option<String>,
    pub reviewed_by: Option<String>,
    #[schema(value_type = Option<chrono::NaiveDateTime>)]
    pub reviewed_at: Option<fubbik_db::timestamp::UtcTimestamp>,
    pub chunks: Vec<RequirementChunk>,
}

impl RequirementDetail {
    pub fn new(req: Requirement, chunks: Vec<RequirementChunk>) -> Self {
        Self {
            id: req.id,
            title: req.title,
            description: req.description,
            steps: req.steps.0,
            order: req.order,
            status: req.status,
            priority: req.priority,
            space_id: req.space_id,
            user_id: req.user_id,
            created_at: req.created_at,
            updated_at: req.updated_at,
            origin: req.origin,
            review_status: req.review_status,
            use_case_id: req.use_case_id,
            reviewed_by: req.reviewed_by,
            reviewed_at: req.reviewed_at,
            chunks,
        }
    }
}

/// `StepVocabularyWarning extends VocabularyWarning { step: number }`
/// (`packages/api/src/requirements/service.ts:28-30`) — a controlled-
/// vocabulary warning plus which step it came from. Fields duplicated
/// (rather than `#[serde(flatten)]`ing `vocabulary::parser::VocabularyWarning`)
/// for the same utoipa-schema-clarity reason as [`RequirementDetail`].
#[derive(serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StepVocabularyWarning {
    pub position: Position,
    #[serde(rename = "type")]
    pub warning_type: WarningType,
    pub word: String,
    pub message: String,
    pub step: i32,
}

/// Shape of `POST /requirements` and `PATCH /requirements/{id}`, matching
/// Node's `{ requirement, warnings, vocabularyWarnings }`
/// (`packages/api/src/requirements/service.ts:133,185`).
#[derive(serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RequirementWithWarnings {
    pub requirement: Requirement,
    pub warnings: Vec<CrossRefWarning>,
    pub vocabulary_warnings: Vec<StepVocabularyWarning>,
}

#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct ReorderResponse {
    pub updated: usize,
}

#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct MessageResponse {
    pub message: String,
}

/// Shape of `POST /requirements/batch`, matching Node's `batchCreateRequirements`
/// return object (`packages/api/src/requirements/batch-service.ts:97-101`).
#[derive(serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BatchCreateResponse {
    pub created: usize,
    pub requirements: Vec<BatchCreatedRequirement>,
    pub use_cases_created: Vec<BatchUseCaseCreated>,
}

#[derive(serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BatchCreatedRequirement {
    pub id: String,
    pub title: String,
    pub use_case_id: Option<String>,
}

#[derive(serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BatchUseCaseCreated {
    pub id: String,
    pub name: String,
    pub parent_id: Option<String>,
}

/// Body of `POST /requirements/{id}/dependencies`
/// (`packages/api/src/requirements/dependency-routes.ts:20-22`).
#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AddDependencyBody {
    pub depends_on_id: String,
}

/// Shape of `GET /requirements/{id}/dependencies`: `{dependsOn,
/// dependedOnBy}`, matching Node's `getDependencies` return object
/// (`packages/db/src/repository/requirement-dependency.ts:39-65`).
#[derive(serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DependencySides {
    pub depends_on: Vec<fubbik_db::repo::requirement_dependency::DependencySummary>,
    pub depended_on_by: Vec<fubbik_db::repo::requirement_dependency::DependencySummary>,
}

/// One node of `GET /requirements/{id}/dependencies/graph`'s `nodes` array,
/// matching Node's inline shape (`packages/api/src/requirements/
/// dependency-service.ts:52-56`).
#[derive(serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DependencyGraphNode {
    pub id: String,
    pub title: String,
    pub status: String,
    pub priority: Option<String>,
    pub is_current: bool,
}

#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct DependencyGraphEdge {
    pub source: String,
    pub target: String,
}

/// Shape of `GET /requirements/{id}/dependencies/graph`: `{nodes, edges}`,
/// matching Node's `getDependencyGraph` return object
/// (`packages/api/src/requirements/dependency-service.ts:61-67`).
#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct DependencyGraph {
    pub nodes: Vec<DependencyGraphNode>,
    pub edges: Vec<DependencyGraphEdge>,
}

/// One element of `POST /requirements/batch`'s validation-failure `errors`
/// array — matches Node's `{ index, step, error }`
/// (`packages/api/src/requirements/batch-service.ts:26-33`): `index` is
/// which entry in the `requirements` array failed, `step`/`error` are
/// `validator::StepError`'s own fields spread in alongside it.
#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct BatchStepError {
    pub index: i32,
    pub step: i32,
    pub error: String,
}
