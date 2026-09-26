use anyhow::{Context, Result, bail};

/// Mirrors the API's camelCase output. The CLI deserialises the wire format,
/// not the database row, so this must match `fubbik_db::repo::chunk::Chunk`'s
/// serde representation rather than its field names.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Chunk {
    pub id: String,
    pub title: String,
    #[serde(rename = "type")]
    pub chunk_type: String,
    pub content: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecapChunk {
    pub id: String,
    pub title: String,
    #[serde(rename = "type")]
    pub chunk_type: String,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub tags: Vec<serde_json::Value>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(untagged)]
enum ChunkResponse {
    Detail { chunk: Chunk },
    Flat(Chunk),
}

impl ChunkResponse {
    fn into_chunk(self) -> Chunk {
        match self {
            Self::Detail { chunk } | Self::Flat(chunk) => chunk,
        }
    }
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct ClaudeMdResponse {
    pub content: String,
    pub chunks: usize,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Proposal {
    pub id: String,
    pub chunk_id: String,
    pub changes: serde_json::Value,
    pub reason: Option<String>,
    pub status: String,
    pub proposed_by: String,
    pub reviewed_by: Option<String>,
    pub reviewed_at: Option<String>,
    pub review_note: Option<String>,
    pub created_at: String,
    #[serde(default)]
    pub chunk_title: Option<String>,
    #[serde(default)]
    pub chunk_type: Option<String>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub id: String,
    pub title: String,
    pub status: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub space_id: Option<String>,
    #[serde(default)]
    pub codebase_name: Option<String>,
    #[serde(default)]
    pub task_total: i64,
    #[serde(default)]
    pub task_done: i64,
    #[serde(default)]
    pub next_action: Option<String>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanTask {
    pub id: String,
    pub plan_id: String,
    pub title: String,
    pub status: String,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct PlanDetail {
    pub plan: Plan,
    pub tasks: Vec<PlanTask>,
    #[serde(default)]
    pub requirements: Vec<serde_json::Value>,
    #[serde(default)]
    pub analyze: serde_json::Value,
    #[serde(default)]
    pub dependencies: Vec<serde_json::Value>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Space {
    pub id: String,
    pub name: String,
    pub kind: String,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Tag {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub chunk_count: i64,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Connection {
    pub id: String,
    pub source_id: String,
    pub target_id: String,
    pub relation: String,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Requirement {
    pub id: String,
    pub title: String,
    pub status: String,
    #[serde(default)]
    pub priority: Option<String>,
    #[serde(default)]
    pub steps: Vec<serde_json::Value>,
}

#[derive(Debug, serde::Deserialize)]
struct RequirementListResponse {
    requirements: Vec<Requirement>,
}

#[derive(Debug, serde::Deserialize)]
struct RequirementMutationResponse {
    requirement: Requirement,
}

/// `GET /api/chunks` returns `{ chunks, total, limit, offset }`, not a bare
/// array — the CLI only needs the rows, so the other fields are dropped
/// here rather than threaded through every caller.
#[derive(Debug, serde::Deserialize)]
struct ChunkListResponse {
    chunks: Vec<Chunk>,
    total: i64,
}

pub struct ChunkPage {
    pub chunks: Vec<Chunk>,
    pub total: i64,
}

#[derive(Debug, serde::Deserialize)]
struct RecapChunkListResponse {
    chunks: Vec<RecapChunk>,
    total: i64,
}

pub struct RecentChunks {
    pub chunks: Vec<RecapChunk>,
    pub total: i64,
}

pub struct Client {
    base: String,
    http: reqwest::Client,
}

mod chunks;
mod context;
mod documents;
mod matrices;
mod plans;
mod requirements;
mod spaces;
mod transport;

pub use matrices::{Matrix, MatrixDimension, MatrixRule, MatrixView};

impl Client {
    pub async fn list_updates(
        &self,
        tag: &str,
        space_id: Option<&str>,
    ) -> Result<serde_json::Value> {
        let mut query = vec![("tag", tag.to_owned())];
        if let Some(value) = space_id {
            query.push(("spaceId", value.to_owned()));
        }
        self.get_json("/api/chunks/updates", &query).await
    }

    pub async fn list_update_tags(&self, space_id: Option<&str>) -> Result<serde_json::Value> {
        let query = space_id
            .map(|value| vec![("spaceId", value.to_owned())])
            .unwrap_or_default();
        self.get_json("/api/chunks/updates/tags", &query).await
    }

    pub async fn health(&self) -> Result<fubbik_contracts::system::HealthResponse> {
        self.get_json("/api/health", &[]).await
    }

    pub async fn knowledge_health(&self, space_id: Option<&str>) -> Result<serde_json::Value> {
        let query = space_id
            .map(|space_id| vec![("spaceId", space_id.to_owned())])
            .unwrap_or_default();
        self.get_json("/api/health/knowledge", &query).await
    }

    pub async fn stats(&self) -> Result<fubbik_contracts::system::Stats> {
        self.get_json("/api/stats", &[]).await
    }

    pub async fn enrich_chunk(&self, id: &str) -> Result<serde_json::Value> {
        self.send_json(
            reqwest::Method::POST,
            &format!("/api/chunks/{id}/enrich"),
            serde_json::Value::Null,
        )
        .await
    }

    pub async fn enrich_all(&self) -> Result<serde_json::Value> {
        self.send_json(
            reqwest::Method::POST,
            "/api/chunks/enrich-all",
            serde_json::Value::Null,
        )
        .await
    }

    pub async fn list_stale(
        &self,
        reason: Option<&str>,
        space_id: Option<&str>,
        limit: u32,
    ) -> Result<Vec<serde_json::Value>> {
        let mut query = vec![("limit", limit.to_string())];
        if let Some(value) = reason {
            query.push(("reason", value.to_owned()));
        }
        if let Some(value) = space_id {
            query.push(("spaceId", value.to_owned()));
        }
        self.get_json("/api/chunks/stale", &query).await
    }

    pub async fn dismiss_stale(&self, id: &str) -> Result<serde_json::Value> {
        self.send_json(
            reqwest::Method::POST,
            &format!("/api/chunks/{id}/dismiss-staleness"),
            serde_json::Value::Null,
        )
        .await
    }

    pub async fn list_proposals(
        &self,
        status: Option<&str>,
        chunk_id: Option<&str>,
        limit: u32,
    ) -> Result<Vec<Proposal>> {
        let mut query = vec![("limit", limit.to_string())];
        if let Some(status) = status {
            query.push(("status", status.to_string()));
        }
        if let Some(chunk_id) = chunk_id {
            query.push(("chunkId", chunk_id.to_string()));
        }
        self.get_json("/api/proposals", &query).await
    }

    pub async fn get_proposal(&self, id: &str) -> Result<Proposal> {
        self.get_json(&format!("/api/proposals/{id}"), &[]).await
    }

    pub async fn review_proposal(
        &self,
        id: &str,
        action: &str,
        note: Option<&str>,
    ) -> Result<Proposal> {
        self.send_json(
            reqwest::Method::POST,
            &format!("/api/proposals/{id}/{action}"),
            serde_json::json!({ "note": note }),
        )
        .await
    }
}
