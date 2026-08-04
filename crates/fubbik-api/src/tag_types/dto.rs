#[derive(serde::Deserialize, utoipa::ToSchema)]
pub struct CreateTagTypeBody {
    pub name: String,
    pub color: Option<String>,
    pub icon: Option<String>,
}

#[derive(serde::Deserialize, utoipa::ToSchema)]
pub struct UpdateTagTypeBody {
    pub name: Option<String>,
    pub color: Option<String>,
    pub icon: Option<String>,
}

/// Shape of every `{ message: "Deleted" }` delete response in this domain,
/// matching Node's convention (`_mutating.md`): deletes return 200, not
/// 204, with this body.
#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct MessageResponse {
    pub message: String,
}
