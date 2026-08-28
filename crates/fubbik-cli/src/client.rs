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

/// `GET /api/chunks` returns `{ chunks, total, limit, offset }`, not a bare
/// array — the CLI only needs the rows, so the other fields are dropped
/// here rather than threaded through every caller.
#[derive(Debug, serde::Deserialize)]
struct ChunkListResponse {
    chunks: Vec<Chunk>,
}

pub struct Client {
    base: String,
    http: reqwest::Client,
}

impl Client {
    pub fn new(base: impl Into<String>) -> Self {
        Self {
            base: base.into(),
            http: reqwest::Client::builder()
                .cookie_store(true)
                .build()
                .expect("http client builds"),
        }
    }

    async fn get_json<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, String)],
    ) -> Result<T> {
        let res = self
            .http
            .get(format!("{}{path}", self.base))
            .query(query)
            .send()
            .await
            .with_context(|| format!("could not reach fubbik at {}", self.base))?;

        if !res.status().is_success() {
            bail!("request to {path} failed with {}", res.status());
        }
        Ok(res.json().await?)
    }

    async fn send_json<T: serde::de::DeserializeOwned>(
        &self,
        method: reqwest::Method,
        path: &str,
        body: serde_json::Value,
    ) -> Result<T> {
        let res = self
            .http
            .request(method, format!("{}{path}", self.base))
            .json(&body)
            .send()
            .await
            .with_context(|| format!("could not reach fubbik at {}", self.base))?;

        if !res.status().is_success() {
            let status = res.status();
            let detail = res.text().await.unwrap_or_default();
            if detail.is_empty() {
                bail!("request to {path} failed with {status}");
            }
            bail!("request to {path} failed with {status}: {detail}");
        }
        Ok(res.json().await?)
    }

    pub async fn list_chunks(
        &self,
        chunk_type: Option<&str>,
        search: Option<&str>,
        limit: u32,
    ) -> Result<Vec<Chunk>> {
        let mut query = vec![("limit", limit.to_string())];
        if let Some(t) = chunk_type {
            query.push(("type", t.to_string()));
        }
        if let Some(s) = search {
            query.push(("search", s.to_string()));
        }
        let res: ChunkListResponse = self.get_json("/api/chunks", &query).await?;
        Ok(res.chunks)
    }

    pub async fn get_chunk(&self, id: &str) -> Result<Chunk> {
        self.get_json(&format!("/api/chunks/{id}"), &[]).await
    }

    pub async fn create_chunk(
        &self,
        title: &str,
        content: &str,
        chunk_type: &str,
    ) -> Result<Chunk> {
        let res = self
            .http
            .post(format!("{}/api/chunks", self.base))
            .json(&serde_json::json!({ "title": title, "content": content, "type": chunk_type }))
            .send()
            .await
            .with_context(|| format!("could not reach fubbik at {}", self.base))?;

        if !res.status().is_success() {
            bail!("create failed with {}", res.status());
        }
        Ok(res.json().await?)
    }

    pub async fn health(&self) -> Result<serde_json::Value> {
        self.get_json("/api/health", &[]).await
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

    pub async fn list_plans(
        &self,
        status: Option<&str>,
        space_id: Option<&str>,
    ) -> Result<Vec<Plan>> {
        let mut query = Vec::new();
        if let Some(status) = status {
            query.push(("status", status.to_string()));
        }
        if let Some(space_id) = space_id {
            query.push(("spaceId", space_id.to_string()));
        }
        self.get_json("/api/plans", &query).await
    }

    pub async fn get_plan(&self, id: &str) -> Result<PlanDetail> {
        self.get_json(&format!("/api/plans/{id}"), &[]).await
    }

    pub async fn create_plan(
        &self,
        title: &str,
        description: Option<&str>,
        space_id: Option<&str>,
        tasks: &[&str],
    ) -> Result<Plan> {
        let tasks: Vec<_> = tasks
            .iter()
            .map(|title| serde_json::json!({ "title": title }))
            .collect();
        self.send_json(
            reqwest::Method::POST,
            "/api/plans",
            serde_json::json!({
                "title": title,
                "description": description,
                "spaceId": space_id,
                "tasks": tasks,
            }),
        )
        .await
    }

    pub async fn update_plan_status(&self, id: &str, status: &str) -> Result<Plan> {
        self.send_json(
            reqwest::Method::PATCH,
            &format!("/api/plans/{id}"),
            serde_json::json!({ "status": status }),
        )
        .await
    }

    pub async fn create_quick_task(
        &self,
        title: &str,
        description: Option<&str>,
        space_id: Option<&str>,
    ) -> Result<Plan> {
        let created = self
            .create_plan(title, description, space_id, &[title])
            .await?;
        self.update_plan_status(&created.id, "in_progress").await
    }

    pub async fn update_task_status(
        &self,
        plan_id: &str,
        task_id: &str,
        status: &str,
    ) -> Result<PlanTask> {
        self.send_json(
            reqwest::Method::PATCH,
            &format!("/api/plans/{plan_id}/tasks/{task_id}"),
            serde_json::json!({ "status": status }),
        )
        .await
    }

    pub async fn set_quick_task_status(&self, plan_id: &str, status: &str) -> Result<PlanTask> {
        let detail = self.get_plan(plan_id).await?;
        let first = detail
            .tasks
            .first()
            .ok_or_else(|| anyhow::anyhow!("plan {plan_id} has no task"))?;
        self.update_task_status(plan_id, &first.id, status).await
    }
}
