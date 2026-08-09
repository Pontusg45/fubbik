use fubbik_db::repo::plan::Plan;

use super::db::{PlanAnalyzeItem, PlanRequirement, PlanTaskChunkWithTitle, PlanTaskDependency};

/// Query params for `GET /api/plans`
/// (`packages/api/src/plans/routes.ts:27-34`): all four optional,
/// `includeArchived` arrives as a string compared `=== "true"` — same
/// not-a-real-boolean quirk as `notifications::dto::ListNotificationsQuery`.
#[derive(serde::Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct ListPlansQuery {
    pub space_id: Option<String>,
    pub status: Option<String>,
    pub requirement_id: Option<String>,
    pub include_archived: Option<String>,
}

impl ListPlansQuery {
    pub fn into_filter(self) -> fubbik_db::repo::plan::ListFilter {
        fubbik_db::repo::plan::ListFilter {
            space_id: self.space_id,
            status: self.status,
            requirement_id: self.requirement_id,
            include_archived: self.include_archived.as_deref() == Some("true"),
        }
    }
}

/// Body of one entry in `POST /api/plans`'s optional `tasks` array
/// (`packages/api/src/plans/routes.ts:67-75`). `acceptanceCriteria` is the
/// legacy `string[]` write shape — normalised to `{text,done}[]` before
/// insert by `normalize_acceptance_criteria`, same as every other write
/// path in this domain.
#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateTaskInput {
    pub title: String,
    pub description: Option<String>,
    pub acceptance_criteria: Option<Vec<String>>,
}

/// Body of `POST /api/plans` (`packages/api/src/plans/routes.ts:62-77`).
#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreatePlanBody {
    pub title: String,
    pub description: Option<String>,
    pub space_id: Option<String>,
    pub requirement_ids: Option<Vec<String>>,
    pub tasks: Option<Vec<CreateTaskInput>>,
    pub metadata: Option<serde_json::Value>,
}

/// Body of `PATCH /api/plans/{id}` (`packages/api/src/plans/routes.ts:104-110`).
///
/// `description` and `space_id` are tri-state, matching Node's
/// `t.Optional(t.Union([t.String(), t.Null()]))` for both: omitted
/// (`None`) leaves the field untouched, explicit `null`
/// (`Some(None)`) clears it, a string (`Some(Some(..))`) sets it — same
/// `deserialize_some` trick as `workspaces::dto::UpdateWorkspaceBody::description`.
/// `status` and `metadata` are plain two-state (Node declares neither with
/// a `t.Null()` union).
#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePlanBody {
    pub title: Option<String>,
    #[serde(default, deserialize_with = "deserialize_some")]
    pub description: Option<Option<String>>,
    pub status: Option<String>,
    #[serde(default, deserialize_with = "deserialize_some")]
    pub space_id: Option<Option<String>>,
    pub metadata: Option<serde_json::Value>,
}

fn deserialize_some<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    T: serde::Deserialize<'de>,
    D: serde::Deserializer<'de>,
{
    T::deserialize(deserializer).map(Some)
}

/// Body of `POST /api/plans/{id}/links`
/// (`packages/api/src/plans/routes.ts:208-214`): `system` defaults to
/// `"url"`, `label` to `null` when omitted — applied in `service::add_link`,
/// not here, since the defaulting happens after the field is already known
/// to be absent (Elysia's `?? "url"` / `?? null`, not a serde default that
/// would collapse "omitted" and "empty string").
#[derive(serde::Deserialize, utoipa::ToSchema)]
pub struct CreateLinkBody {
    pub url: String,
    pub system: Option<String>,
    pub label: Option<String>,
}

/// Shape of every `{ ok: true }` response in this domain — Node's plans
/// routes discard the delete/unlink Effect's own result and return this
/// literal instead (`_mutating.md`), unlike most other domains' `{ message:
/// "Deleted" }` convention.
#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct OkResponse {
    pub ok: bool,
}

impl Default for OkResponse {
    fn default() -> Self {
        Self { ok: true }
    }
}

/// One normalised acceptance-criterion entry. `acceptanceCriteria` was
/// originally stored as `string[]`; both the legacy shape and the newer
/// `{text,done}[]` shape are read and always normalised to this object
/// shape on the way out — matching Node's `normaliseAcceptanceCriteria`
/// (`packages/api/src/plans/service.ts:110-122`).
#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct AcceptanceCriterion {
    pub text: String,
    pub done: bool,
}

/// Reproduces Node's `normaliseAcceptanceCriteria` exactly: a bare string
/// becomes `{text: item, done: false}`; an object with a `text` key keeps
/// its `done` flag (defaulting to `false` if absent or not a bool);
/// anything else (a stray number, `null`, ...) becomes `{text: "", done:
/// false}`. Not `Vec::new()` on a non-array input's *elements* — only a
/// non-array `raw` itself yields the empty vec (`Array.isArray(raw)` guard).
pub fn normalize_acceptance_criteria(raw: &serde_json::Value) -> Vec<AcceptanceCriterion> {
    let Some(arr) = raw.as_array() else {
        return vec![];
    };
    arr.iter()
        .map(|item| match item {
            serde_json::Value::String(s) => AcceptanceCriterion {
                text: s.clone(),
                done: false,
            },
            serde_json::Value::Object(obj) if obj.contains_key("text") => AcceptanceCriterion {
                text: match obj.get("text") {
                    Some(serde_json::Value::String(s)) => s.clone(),
                    Some(other) => other.to_string(),
                    None => String::new(),
                },
                done: obj.get("done").and_then(|v| v.as_bool()).unwrap_or(false),
            },
            _ => AcceptanceCriterion {
                text: String::new(),
                done: false,
            },
        })
        .collect()
}

/// Normalises a write-side `Vec<String>` (the legacy shape Node's create
/// body still accepts) into the stored `{text,done}[]` JSON, matching
/// `normaliseAcceptanceCriteria(t.acceptanceCriteria ?? [])` at plan-create
/// time (`packages/api/src/plans/service.ts:150`).
pub fn acceptance_criteria_for_write(items: &[String]) -> serde_json::Value {
    serde_json::Value::Array(
        items
            .iter()
            .map(|text| serde_json::json!({ "text": text, "done": false }))
            .collect(),
    )
}

/// A task in the `GET /api/plans/{id}` detail envelope: the raw
/// `plan_task` row's fields, plus `acceptanceCriteria` normalised (not the
/// raw stored JSON) and the task's linked chunks — matching Node's
/// `tasksWithChunks` (`packages/api/src/plans/service.ts:89-94`).
#[derive(serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TaskDetail {
    pub id: String,
    pub plan_id: String,
    pub title: String,
    pub description: Option<String>,
    pub acceptance_criteria: Vec<AcceptanceCriterion>,
    pub status: String,
    pub order: i32,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub created_at: fubbik_db::timestamp::UtcTimestamp,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub updated_at: fubbik_db::timestamp::UtcTimestamp,
    #[schema(value_type = std::collections::HashMap<String, serde_json::Value>)]
    pub metadata: serde_json::Value,
    pub chunks: Vec<PlanTaskChunkWithTitle>,
}

/// The five fixed analyze-item buckets, always present even when empty —
/// confirmed against real bytes in `tests/fixtures/node-contract-2c/
/// plans-detail-analyze.json` (Q1 in `_questions.md`): `{chunk:[],file:[],
/// risk:[],assumption:[],question:[]}`, never an omitted key. Matches
/// Node's `groupByKind` (`packages/api/src/plans/analyze.ts:14-28`), which
/// initialises all five keys unconditionally before bucketing.
#[derive(serde::Serialize, utoipa::ToSchema, Default)]
pub struct AnalyzeGrouped {
    pub chunk: Vec<PlanAnalyzeItem>,
    pub file: Vec<PlanAnalyzeItem>,
    pub risk: Vec<PlanAnalyzeItem>,
    pub assumption: Vec<PlanAnalyzeItem>,
    pub question: Vec<PlanAnalyzeItem>,
}

impl AnalyzeGrouped {
    pub fn from_items(items: Vec<PlanAnalyzeItem>) -> Self {
        let mut grouped = Self::default();
        for item in items {
            match item.kind.as_str() {
                "chunk" => grouped.chunk.push(item),
                "file" => grouped.file.push(item),
                "risk" => grouped.risk.push(item),
                "assumption" => grouped.assumption.push(item),
                "question" => grouped.question.push(item),
                // An unrecognised `kind` is dropped, matching Node's
                // `isAnalyzeKind` guard (`plans/service.ts:15-17,83-87`),
                // which silently skips anything outside the five known
                // kinds rather than erroring.
                _ => {}
            }
        }
        grouped
    }
}

/// Response envelope of `GET /api/plans/{id}`
/// (`tests/fixtures/node-contract-2c/plans-detail.json`) — `{plan,
/// requirements, analyze, tasks, dependencies}`, NOT the bare `Plan` that
/// `GET /api/plans` (as a rollup row) and every plan-mutating endpoint
/// return.
#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct PlanDetail {
    pub plan: Plan,
    pub requirements: Vec<PlanRequirement>,
    pub analyze: AnalyzeGrouped,
    pub tasks: Vec<TaskDetail>,
    pub dependencies: Vec<PlanTaskDependency>,
}
