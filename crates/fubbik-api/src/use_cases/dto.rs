/// Query params of `GET /api/use-cases` (`packages/api/src/use-cases/routes.ts:10-16`):
/// `spaceId: t.Optional(t.String())`, an exact-match filter applied only
/// when present.
#[derive(serde::Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct ListUseCasesQuery {
    pub space_id: Option<String>,
}

/// Body of `POST /api/use-cases` (`packages/api/src/use-cases/routes.ts:26-33`).
/// `order` is deliberately absent — Node's create body doesn't accept it,
/// so a new use case always gets the DB's `order` default (`0`); it can
/// only be set afterward via `PATCH`.
#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateUseCaseBody {
    pub name: String,
    pub description: Option<String>,
    pub space_id: Option<String>,
    pub parent_id: Option<String>,
}

/// Body of `PATCH /api/use-cases/{id}` (`packages/api/src/use-cases/routes.ts:50-58`).
///
/// `description` is tri-state, matching Node's `t.Optional(t.Union([t.String(),
/// t.Null()]))`: omitted (`None`) leaves it untouched, explicit `null`
/// (`Some(None)`) clears it, a string (`Some(Some(..))`) sets it. `parentId`
/// is tri-state the same way, matching `t.Optional(t.Union([t.String(),
/// t.Null()]))` — explicit `null` detaches this use case from its parent
/// with no re-nesting validation (see `service::update`'s doc comment for
/// exactly which branch of Node's validation that skips). `order` is plain
/// two-state — the DB column is `NOT NULL DEFAULT 0`, so there is no way to
/// clear it, only set or leave it.
#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateUseCaseBody {
    pub name: Option<String>,
    #[serde(default, deserialize_with = "deserialize_some")]
    pub description: Option<Option<String>>,
    pub order: Option<i32>,
    #[serde(default, deserialize_with = "deserialize_some")]
    pub parent_id: Option<Option<String>>,
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
