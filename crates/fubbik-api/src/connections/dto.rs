/// Matches Node's body schema exactly (`packages/api/src/connections/routes.ts:22-27`):
/// `sourceId`/`targetId`/`relation` required, `origin` optional and
/// constrained to the two literals Node accepts (`t.Union([t.Literal("human"),
/// t.Literal("ai")])`) — anything else fails validation before reaching the
/// service layer, matching Node's Elysia schema rejecting the request
/// outright rather than letting an arbitrary `origin` string through.
#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateConnectionBody {
    pub source_id: String,
    pub target_id: String,
    pub relation: String,
    pub origin: Option<Origin>,
}

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

/// Shape of every `{ message: "Deleted" }` delete response in this domain,
/// matching Node's convention (`_mutating.md`): deletes return 200, not
/// 204, with this body.
#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct MessageResponse {
    pub message: String,
}
