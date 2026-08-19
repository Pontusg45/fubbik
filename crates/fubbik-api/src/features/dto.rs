//! Wire types for the `features` domain.
//!
//! Every `Deserialize` type here carries `#[serde(rename_all =
//! "camelCase")]` where it has a multi-word field: the wire is camelCase
//! (`spaceId`, `featureIds`, `spaceIds`), and serde *silently ignores* an
//! unknown key rather than rejecting it — a missing rename on a query
//! struct therefore produces an endpoint that looks healthy while quietly
//! dropping the filter.

use fubbik_db::repo::feature::{DeltaWithChunk, Feature, FeatureSpace};

/// Query of `GET /features` (`packages/api/src/features/routes.ts:23-27`).
/// All three are `t.Optional(t.String())` — `status` is **free text**, not
/// an enum, and is compared with a plain `=`; `search` is a raw
/// `ILIKE '%…%'`. Each is truthy-checked in Node, so an empty string means
/// "no filter" (applied in `feature::list`).
#[derive(serde::Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct ListFeaturesQuery {
    pub space_id: Option<String>,
    pub status: Option<String>,
    pub search: Option<String>,
}

/// Body of `POST /features` (`packages/api/src/features/routes.ts:44-50`).
///
/// Node declares `maxLength` on `name` (100), `description` (1000) and
/// `color` (7); this port does not enforce those bounds — the same accepted
/// divergence as `favorites::dto::CreateFavoriteBody`. `color` has no
/// format validation in Node either (it is not checked to be a hex code).
/// `priority` is `t.Number()` in Node, i.e. a JS float; it is `i32` here
/// because the column is `integer` (a fractional priority is a 500 in Node,
/// a 400 here).
#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateFeatureBody {
    pub name: String,
    pub description: Option<String>,
    pub priority: Option<i32>,
    pub color: Option<String>,
    pub space_ids: Option<Vec<String>>,
}

/// The three statuses a client may PATCH onto a feature
/// (`packages/api/src/features/routes.ts:87`). This is one of the rare
/// cases where a status field really *is* constrained in Node — the route
/// schema is `t.Union([t.Literal("active"), t.Literal("inactive"),
/// t.Literal("archived")])`, so `"merged"` cannot be set through the API at
/// all; only `POST /features/{id}/merge` writes it. The database column
/// itself is unconstrained `text`, which is why
/// `fubbik_db::repo::feature::Feature::status` stays a `String` on the way
/// out — the *input* is narrower than the *output*, deliberately.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum FeatureStatus {
    Active,
    Inactive,
    Archived,
}

impl FeatureStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            FeatureStatus::Active => "active",
            FeatureStatus::Inactive => "inactive",
            FeatureStatus::Archived => "archived",
        }
    }
}

/// Body of `PATCH /features/{id}` (`packages/api/src/features/routes.ts:
/// 83-90`). `description` and `color` are tri-state (`None` = untouched,
/// `Some(None)` = clear, `Some(Some(v))` = set) because Node types both as
/// `t.Optional(t.Union([t.String(), t.Null()]))`. `spaceIds` is stripped
/// off before the row update and applied separately, matching
/// `packages/api/src/features/service.ts:112`.
#[derive(Default, serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateFeatureBody {
    pub name: Option<String>,
    #[serde(default, deserialize_with = "deserialize_some")]
    pub description: Option<Option<String>>,
    pub priority: Option<i32>,
    pub status: Option<FeatureStatus>,
    #[serde(default, deserialize_with = "deserialize_some")]
    pub color: Option<Option<String>>,
    pub space_ids: Option<Vec<String>>,
}

fn deserialize_some<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    T: serde::Deserialize<'de>,
    D: serde::Deserializer<'de>,
{
    T::deserialize(deserializer).map(Some)
}

/// Body of `PUT /features/active` (`packages/api/src/features/routes.ts:
/// 66-68`).
#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SetActiveFeaturesBody {
    pub feature_ids: Vec<String>,
}

/// Body of `POST /features/{id}/reorder` (`packages/api/src/features/
/// routes.ts:118-120`). Named `ReorderFeatureBody` rather than
/// `ReorderBody` because `requirements::dto::ReorderBody` already occupies
/// that schema name in utoipa's single flat namespace.
#[derive(serde::Deserialize, utoipa::ToSchema)]
pub struct ReorderFeatureBody {
    pub priority: i32,
}

/// Body of `PUT /chunks/{id}/deltas/{featureId}`
/// (`packages/api/src/features/routes.ts:137-139`).
///
/// `delta` is `t.Record(t.String(), t.Unknown())` in Node — an *object*,
/// with arbitrary values — so it is a `serde_json::Map` here rather than a
/// bare `Value`: a JSON array or string for `delta` is rejected, matching
/// Node. Which keys are permitted is checked in the service, not here, so
/// the rejection message can name the offending fields the way Node's
/// `validateDelta` does.
#[derive(serde::Deserialize, utoipa::ToSchema)]
pub struct UpsertDeltaBody {
    #[schema(value_type = serde_json::Value)]
    pub delta: serde_json::Map<String, serde_json::Value>,
}

/// Response of `GET /features/{id}` — Node's `getFeatureDetail` resolves an
/// `Effect.all({ feature, spaces, deltas })`, so the three land as sibling
/// keys rather than the feature being flattened
/// (`packages/api/src/features/service.ts:72-83`).
/// `GET /features/{id}/deltas` returns just the `deltas` member of this
/// same value.
#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct FeatureDetail {
    pub feature: Feature,
    pub spaces: Vec<FeatureSpace>,
    pub deltas: Vec<DeltaWithChunk>,
}

/// `{ message: "..." }`, this domain's shape for `DELETE /features/{id}`
/// ("Deleted"), `PUT /features/active` ("Active features updated"),
/// `POST /features/{id}/merge` ("Feature merged") and
/// `DELETE /chunks/{id}/deltas/{featureId}` ("Delta deleted").
#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct MessageResponse {
    pub message: String,
}
