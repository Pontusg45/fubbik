use fubbik_db::repo::workspace::{Workspace, WorkspaceSpaceSummary};

/// Body of `POST /api/workspaces` (`packages/api/src/workspaces/routes.ts:11-30`):
/// `name` (required, trimmed and validated non-blank at the service layer —
/// see `service::create`) and `description` (optional, passed through
/// as-is).
#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateWorkspaceBody {
    pub name: String,
    pub description: Option<String>,
}

/// Body of `PATCH /api/workspaces/{id}` (`packages/api/src/workspaces/routes.ts:36-50`).
///
/// `description` is tri-state, matching Node's `t.Optional(t.Union([t.String(),
/// t.Null()]))`: omitted (`None`) leaves it untouched, explicit `null`
/// (`Some(None)`) clears it, a string (`Some(Some(..))`) sets it — same
/// `deserialize_some` trick as `spaces::dto::UpdateSpaceBody::description`,
/// needed because a plain `Option<Option<T>>` collapses "omitted" and
/// "explicit null" to the same `None` without it.
#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateWorkspaceBody {
    pub name: Option<String>,
    #[serde(default, deserialize_with = "deserialize_some")]
    pub description: Option<Option<String>>,
}

fn deserialize_some<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    T: serde::Deserialize<'de>,
    D: serde::Deserializer<'de>,
{
    T::deserialize(deserializer).map(Some)
}

/// Body of `POST /api/workspaces/{id}/spaces` (`packages/api/src/workspaces/routes.ts:60-78`).
#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AddSpaceBody {
    pub space_id: String,
}

/// Response shape of `GET /api/workspaces/{id}`, matching Node's spread
/// `{ ...found, spaces }` (`packages/api/src/workspaces/service.ts:19-28`)
/// — the workspace row's own fields at the top level, plus a `spaces`
/// array, NOT nested under a `workspace` key (contrast
/// `fubbik_db::repo::space::SpaceDetail`, which nests `{ space, code }`).
/// See `tests/fixtures/node-contract-2b/workspaces-detail.json`.
#[derive(serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceDetail {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub user_id: String,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub created_at: fubbik_db::timestamp::UtcTimestamp,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub updated_at: fubbik_db::timestamp::UtcTimestamp,
    pub spaces: Vec<WorkspaceSpaceSummary>,
}

impl WorkspaceDetail {
    pub fn new(w: Workspace, spaces: Vec<WorkspaceSpaceSummary>) -> Self {
        Self {
            id: w.id,
            name: w.name,
            description: w.description,
            user_id: w.user_id,
            created_at: w.created_at,
            updated_at: w.updated_at,
            spaces,
        }
    }
}

/// Shape of every `{ message: "Deleted" }` response in this domain,
/// matching Node's convention (`_mutating.md`).
#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct MessageResponse {
    pub message: String,
}
