use std::collections::HashMap;

use fubbik_db::repo::saved_graph::Position;

/// Query params of `GET /api/saved-graphs`
/// (`packages/api/src/saved-graphs/routes.ts:8-14`): `ctx.query` is cast to
/// `{ spaceId?: string }` with no schema validation, so this stays a plain
/// optional string, not a `t.Optional(t.String())`-derived numeric/bool
/// coercion the way other query params in this crate have.
#[derive(serde::Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct ListSavedGraphsQuery {
    pub space_id: Option<String>,
}

/// Body of `POST /api/saved-graphs`
/// (`packages/api/src/saved-graphs/routes.ts:29-37`). `chunkIds` and
/// `positions` are structurally validated by Elysia's schema (`t.Array(t.String())`,
/// `t.Record(t.String(), t.Object({ x: t.Number(), y: t.Number() }))`) —
/// not opaque JSON the way `collection.filter` / `saved_query.query` are —
/// so this keeps the same concrete Rust types end to end rather than
/// falling back to `serde_json::Value`. `spaceId` accepts both omission and
/// explicit `null` (`t.Optional(t.Union([t.String(), t.Null()]))`); both
/// produce the same `NULL` insert in Node's `.values(params)` (an
/// `undefined` key and an explicit `null` value behave identically for a
/// nullable Drizzle column), so a plain `Option<String>` — not the
/// `deserialize_some` tri-state — is correct here, unlike `description` on
/// the PATCH body below.
#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateSavedGraphBody {
    pub name: String,
    pub description: Option<String>,
    pub chunk_ids: Vec<String>,
    pub positions: HashMap<String, Position>,
    pub layout_algorithm: Option<String>,
    pub space_id: Option<String>,
}

/// Body of `PATCH /api/saved-graphs/{id}`
/// (`packages/api/src/saved-graphs/routes.ts:53-59`). `description` is
/// tri-state, matching Node's `t.Optional(t.Union([t.String(), t.Null()]))`:
/// omitted (`None`) leaves it untouched, explicit `null` (`Some(None)`)
/// clears it, a string (`Some(Some(..))`) sets it — same `deserialize_some`
/// trick as `workspaces::dto::UpdateWorkspaceBody::description`. Every
/// other field here is plain two-state (`t.Optional(...)` with no
/// `t.Null()` variant) — `spaceId` is deliberately absent, matching Node:
/// there is no way to change a saved graph's space through this endpoint.
#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSavedGraphBody {
    pub name: Option<String>,
    #[serde(default, deserialize_with = "deserialize_some")]
    pub description: Option<Option<String>>,
    pub chunk_ids: Option<Vec<String>>,
    pub positions: Option<HashMap<String, Position>>,
    pub layout_algorithm: Option<String>,
}

fn deserialize_some<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    T: serde::Deserialize<'de>,
    D: serde::Deserializer<'de>,
{
    T::deserialize(deserializer).map(Some)
}

/// Shape of the `{ message: "Deleted" }` response, matching every other
/// domain's delete convention (`_mutating.md`).
#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct MessageResponse {
    pub message: String,
}
