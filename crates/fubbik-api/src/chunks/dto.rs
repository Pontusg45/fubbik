use fubbik_db::repo::chunk::Sort;

#[derive(serde::Deserialize, utoipa::ToSchema)]
pub struct CreateChunkBody {
    pub title: String,
    #[serde(default)]
    pub content: String,
    #[serde(rename = "type")]
    pub chunk_type: Option<String>,
    pub rationale: Option<String>,
}

#[derive(serde::Deserialize, utoipa::ToSchema)]
pub struct UpdateChunkBody {
    pub title: Option<String>,
    pub content: Option<String>,
    #[serde(rename = "type")]
    pub chunk_type: Option<String>,
    pub rationale: Option<String>,
    pub consequences: Option<String>,
}

/// Query params arrive as strings from the web client, matching the Elysia
/// route's `t.Optional(t.String())` shape, so numeric fields parse leniently.
#[derive(serde::Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct ListChunksQuery {
    #[serde(rename = "type")]
    pub chunk_type: Option<String>,
    pub search: Option<String>,
    pub origin: Option<String>,
    pub review_status: Option<String>,
    pub sort: Option<Sort>,
    pub limit: Option<String>,
    pub offset: Option<String>,
}

impl ListChunksQuery {
    pub fn into_params(self) -> fubbik_db::repo::chunk::ListParams {
        fubbik_db::repo::chunk::ListParams {
            chunk_type: self.chunk_type,
            search: self.search,
            origin: self.origin,
            review_status: self.review_status,
            sort: self.sort.unwrap_or_default(),
            limit: self.limit.and_then(|s| s.parse().ok()).unwrap_or(50),
            offset: self.offset.and_then(|s| s.parse().ok()).unwrap_or(0),
        }
    }
}
