#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChunkIdBody {
    pub chunk_id: String,
}

#[derive(serde::Deserialize, utoipa::ToSchema)]
pub struct GenerateBody {
    pub prompt: String,
}

#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct SummaryResponse {
    pub summary: String,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, utoipa::ToSchema)]
pub struct AiConnectionSuggestion {
    pub id: String,
    pub relation: String,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, utoipa::ToSchema)]
pub struct GeneratedChunk {
    pub title: String,
    pub content: String,
    #[serde(rename = "type")]
    pub chunk_type: String,
    pub tags: Vec<String>,
}

#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StructureRequirementBody {
    pub description: String,
    /// `spaceId` is the current web contract. `codebaseId` remains accepted
    /// so callers of the legacy route do not break during the rename.
    #[serde(default, alias = "codebaseId")]
    pub space_id: Option<String>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum AiStepKeyword {
    Given,
    When,
    Then,
    And,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, utoipa::ToSchema)]
pub struct AiRequirementStep {
    pub keyword: AiStepKeyword,
    pub text: String,
}

#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
pub struct StructuredRequirement {
    pub steps: Vec<AiRequirementStep>,
}
