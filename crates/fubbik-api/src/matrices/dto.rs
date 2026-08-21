//! Request and response shapes for the matrices domain.
//!
//! Every `maxLength` / literal-union in Node lives on its Elysia route schema
//! and is enforced before the handler runs, so there is nothing to port from
//! its service layer — the constraints simply vanish unless re-expressed.
//! They are validated in `service.rs`, not modelled as serde enums, for the
//! reason spelled out on [`CreateMatrixBody::layer`].

use fubbik_db::repo::behavior_matrix as repo;

/// Distinguishes "key absent" from an explicit `null` for the tri-state
/// PATCH fields. serde collapses both to `None` by default, which would make
/// "clear this field" unreachable from the UI while every set-a-value test
/// still passed — the same trap `chunks::dto`'s `summary` documents.
fn double_option<'de, T, D>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    T: serde::Deserialize<'de>,
    D: serde::Deserializer<'de>,
{
    serde::Deserialize::deserialize(deserializer).map(Some)
}

#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateMatrixBody {
    pub name: String,
    /// `invariant | contract`. Kept a `String` and checked in the service
    /// rather than modelled as a serde enum: the column is free `text` with
    /// no CHECK, older rows may hold anything, and a serde enum would reject
    /// with a parse error instead of a message naming the field. Same
    /// disposition as `chunks::routes::FileRefEntry::relation`.
    pub layer: String,
    pub description: Option<String>,
    pub space_id: Option<String>,
}

#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateMatrixBody {
    pub name: Option<String>,
    #[serde(default, deserialize_with = "double_option")]
    #[schema(value_type = Option<String>)]
    pub description: Option<Option<String>>,
}

#[derive(serde::Deserialize, utoipa::ToSchema)]
pub struct DimensionBody {
    pub name: String,
}

#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReorderDimensionsBody {
    pub dimension_ids: Vec<String>,
}

#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReorderRulesBody {
    pub rule_ids: Vec<String>,
}

#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateRuleBody {
    pub title: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub rationale: Option<String>,
    pub alternatives: Option<String>,
    pub consequences: Option<String>,
    pub counterexample: Option<String>,
}

/// Every field but `title` is tri-state — Node types all six as `T | null`.
#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateRuleBody {
    pub title: Option<String>,
    #[serde(default, deserialize_with = "double_option")]
    #[schema(value_type = Option<String>)]
    pub description: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    #[schema(value_type = Option<String>)]
    pub category: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    #[schema(value_type = Option<String>)]
    pub rationale: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    #[schema(value_type = Option<String>)]
    pub alternatives: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    #[schema(value_type = Option<String>)]
    pub consequences: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    #[schema(value_type = Option<String>)]
    pub counterexample: Option<Option<String>>,
}

#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ToggleCellBody {
    pub rule_id: String,
    pub dimension_id: String,
}

#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LinkRequirementBody {
    pub requirement_id: String,
}

#[derive(serde::Deserialize, utoipa::ToSchema)]
pub struct LinkCodeBody {
    /// `file | symbol | test` — see [`CreateMatrixBody::layer`].
    pub kind: String,
    #[serde(rename = "ref")]
    pub code_ref: String,
}

#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TestResultBody {
    pub test_ref: String,
    /// `pass | fail`.
    pub status: String,
    pub detail: Option<String>,
}

#[derive(serde::Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct ListMatricesQuery {
    pub space_id: Option<String>,
    pub layer: Option<String>,
}

#[derive(serde::Deserialize, utoipa::IntoParams)]
pub struct BehaviorsForFileQuery {
    pub path: String,
}

// ---------------------------------------------------------------------------
// Responses
// ---------------------------------------------------------------------------

/// `GET /api/matrices/{id}` — the matrix plus its axes, not a flattened row.
#[derive(serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MatrixDetail {
    pub matrix: repo::BehaviorMatrix,
    pub dimensions: Vec<repo::BehaviorDimension>,
    pub rules: Vec<repo::BehaviorRule>,
}

/// Computed status of one cell. Derived on read rather than stored, so it
/// always reflects current requirement statuses and test results.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum CellStatus {
    Specified,
    Unspecified,
    Violated,
    Verified,
}

#[derive(serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ViewCell {
    pub id: String,
    pub status: CellStatus,
    pub requirement_count: i64,
    pub code_count: i64,
    pub passing_test_count: i64,
    pub failing_test_count: i64,
}

#[derive(serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ViewSummary {
    pub specified: i64,
    pub unspecified: i64,
    pub violated: i64,
    pub verified: i64,
    pub total: i64,
}

/// `GET /api/matrices/{id}/view`.
///
/// `cells` is keyed `"{ruleId}:{dimensionId}"`, matching Node exactly — the
/// grid UI looks a cell up by its coordinates rather than scanning a list.
/// A missing key means no cell exists at that intersection, which is
/// distinct from a cell whose status is `unspecified`.
#[derive(serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MatrixView {
    pub matrix: repo::BehaviorMatrix,
    pub dimensions: Vec<repo::BehaviorDimension>,
    pub rules: Vec<repo::BehaviorRule>,
    #[schema(value_type = std::collections::HashMap<String, ViewCell>)]
    pub cells: std::collections::HashMap<String, ViewCell>,
    pub summary: ViewSummary,
}

/// `PUT /api/matrices/{id}/cells` — which way the toggle went, plus the row
/// that was created or removed.
#[derive(serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ToggleCellResponse {
    /// `created | deleted`.
    pub action: String,
    pub cell: repo::BehaviorCell,
}

/// The `{ message }` shape Node's deletes and unlinks answer with.
#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct MatrixMessage {
    pub message: String,
}
