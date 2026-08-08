use fubbik_db::repo::collection::CollectionFilter;

/// Body of `POST /api/collections`
/// (`packages/api/src/collections/routes.ts:23-44`). `filter` is required
/// as a whole object but every one of its nine keys is itself optional —
/// see `CollectionFilter`'s doc comment.
#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateCollectionBody {
    pub name: String,
    pub description: Option<String>,
    pub filter: CollectionFilter,
    pub space_id: Option<String>,
}

/// Body of `PATCH /api/collections/{id}`
/// (`packages/api/src/collections/routes.ts:45-60`). All three fields are
/// plain two-state `Option<T>` (omitted = untouched, present = set) — this
/// schema has no `t.Null()` variant on any field, unlike
/// `workspaces::dto::UpdateWorkspaceBody::description`, so there is no way
/// to explicitly clear `description` through this endpoint. `filter`, when
/// provided, **replaces** the stored filter wholesale rather than merging
/// it — see `fubbik_db::repo::collection::CollectionPatch`'s doc comment.
/// `spaceId` is deliberately absent from this body: Node's schema doesn't
/// declare it either, so a collection's space is immutable after creation.
#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCollectionBody {
    pub name: Option<String>,
    pub description: Option<String>,
    pub filter: Option<CollectionFilter>,
}

/// Shape of the `{ message: "Deleted" }` response, matching every other
/// domain's delete convention (`_mutating.md`).
#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct MessageResponse {
    pub message: String,
}
