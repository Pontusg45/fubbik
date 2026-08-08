/// Body of `POST /api/favorites` (`packages/api/src/favorites/routes.ts:12-14`):
/// `chunkId` only, a plain `String` with no length cap. Node's schema
/// declares `t.String({ maxLength: 100 })`, but this port does not enforce
/// that bound — an accepted, deliberate divergence (a `chunkId` over 100
/// chars simply fails the downstream chunk-ownership lookup instead of
/// being rejected up front), not something silently missing.
#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateFavoriteBody {
    pub chunk_id: String,
}

/// One entry of `PUT /api/favorites/reorder`'s bare-array body
/// (`packages/api/src/favorites/routes.ts:39-44`). The body is a plain
/// `t.Array(...)`, not wrapped in an object — see `ReorderBody` below.
#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReorderEntry {
    pub chunk_id: String,
    pub order: i32,
}

/// Shape of every `{ message: "..." }` response in this domain, matching
/// Node's convention (`_mutating.md`): delete (200, `"Deleted"`) and
/// reorder (200, `"Reordered"`) both return this bare object.
#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct MessageResponse {
    pub message: String,
}
