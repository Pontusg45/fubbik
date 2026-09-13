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
                .connect_timeout(std::time::Duration::from_secs(3))
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .expect("http client builds"),
        }
    }

    pub fn base_url(&self) -> &str {
        &self.base
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

    async fn get_text(&self, path: &str, query: &[(&str, String)]) -> Result<String> {
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
        Ok(res.text().await?)
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
        let response: ChunkResponse = self.get_json(&format!("/api/chunks/{id}"), &[]).await?;
        Ok(response.into_chunk())
    }

    pub async fn create_chunk(
        &self,
        title: &str,
        content: &str,
        chunk_type: &str,
        tags: &[String],
        spaces: &[String],
    ) -> Result<Chunk> {
        let res = self
            .http
            .post(format!("{}/api/chunks", self.base))
            .json(&serde_json::json!({
                "title": title,
                "content": content,
                "type": chunk_type,
                "tags": tags,
                "spaceIds": spaces,
            }))
            .send()
            .await
            .with_context(|| format!("could not reach fubbik at {}", self.base))?;

        if !res.status().is_success() {
            bail!("create failed with {}", res.status());
        }
        Ok(res.json().await?)
    }

    pub async fn update_chunk(
        &self,
        id: &str,
        title: Option<&str>,
        content: Option<&str>,
        chunk_type: Option<&str>,
        tags: Option<&[String]>,
        spaces: Option<&[String]>,
    ) -> Result<Chunk> {
        let mut body = serde_json::Map::new();
        if let Some(value) = title {
            body.insert("title".into(), value.into());
        }
        if let Some(value) = content {
            body.insert("content".into(), value.into());
        }
        if let Some(value) = chunk_type {
            body.insert("type".into(), value.into());
        }
        if let Some(value) = tags {
            body.insert("tags".into(), serde_json::to_value(value)?);
        }
        if let Some(value) = spaces {
            body.insert("spaceIds".into(), serde_json::to_value(value)?);
        }
        if body.is_empty() {
            bail!("nothing to update; provide at least one field");
        }
        self.send_json(
            reqwest::Method::PATCH,
            &format!("/api/chunks/{id}"),
            body.into(),
        )
        .await
    }

    pub async fn delete_chunk(&self, id: &str) -> Result<serde_json::Value> {
        self.send_json(
            reqwest::Method::DELETE,
            &format!("/api/chunks/{id}"),
            serde_json::Value::Null,
        )
        .await
    }

    pub async fn export_context(
        &self,
        space: Option<&str>,
        max_tokens: usize,
        format: &str,
        for_path: Option<&str>,
    ) -> Result<serde_json::Value> {
        let mut query = vec![
            ("maxTokens", max_tokens.to_string()),
            ("format", format.to_string()),
        ];
        if let Some(value) = space {
            query.push(("spaceId", value.to_string()));
        }
        if let Some(value) = for_path {
            query.push(("forPath", value.to_string()));
        }
        self.get_json("/api/chunks/export/context", &query).await
    }

    pub async fn context_for_file(
        &self,
        path: &str,
        space: Option<&str>,
        max_tokens: usize,
        format: &str,
    ) -> Result<serde_json::Value> {
        let mut query = vec![
            ("path", path.to_string()),
            ("maxTokens", max_tokens.to_string()),
            ("format", format.to_string()),
        ];
        if let Some(value) = space {
            query.push(("spaceId", value.to_string()));
        }
        self.get_json("/api/context/for-file", &query).await
    }

    pub async fn claude_md(
        &self,
        space: Option<&str>,
        tag: Option<&str>,
        max_tokens: usize,
    ) -> Result<ClaudeMdResponse> {
        let mut query = vec![("maxTokens", max_tokens.to_string())];
        if let Some(value) = space {
            query.push(("spaceId", value.to_string()));
        }
        if let Some(value) = tag {
            query.push(("tag", value.to_string()));
        }
        self.get_json("/api/chunks/export/claude-md", &query).await
    }

    pub async fn health(&self) -> Result<serde_json::Value> {
        self.get_json("/api/health", &[]).await
    }

    pub async fn list_spaces(&self) -> Result<Vec<Space>> {
        self.get_json("/api/spaces", &[]).await
    }

    pub async fn resolve_space(&self, reference: Option<&str>) -> Result<Option<String>> {
        let Some(reference) = reference else {
            return Ok(None);
        };
        self.list_spaces()
            .await?
            .into_iter()
            .find(|space| space.id == reference || space.name == reference)
            .map(|space| Some(space.id))
            .ok_or_else(|| anyhow::anyhow!("space {reference:?} not found"))
    }

    pub async fn resolve_spaces(&self, references: &[String]) -> Result<Vec<String>> {
        if references.is_empty() {
            return Ok(Vec::new());
        }
        let spaces = self.list_spaces().await?;
        references
            .iter()
            .map(|reference| {
                spaces
                    .iter()
                    .find(|space| space.id == *reference || space.name == *reference)
                    .map(|space| space.id.clone())
                    .ok_or_else(|| anyhow::anyhow!("space {reference:?} not found"))
            })
            .collect()
    }

    pub async fn create_space(
        &self,
        name: &str,
        local_path: Option<&str>,
        remote_url: Option<&str>,
    ) -> Result<Space> {
        self.send_json(
            reqwest::Method::POST,
            "/api/spaces",
            serde_json::json!({
                "name": name,
                "kind": "code",
                "localPaths": local_path.map(|path| vec![path]),
                "remoteUrl": remote_url,
            }),
        )
        .await
    }

    pub async fn detect_space(
        &self,
        local_path: Option<&str>,
        remote_url: Option<&str>,
    ) -> Result<Option<Space>> {
        let mut query = Vec::new();
        if let Some(path) = local_path {
            query.push(("localPath", path.to_owned()));
        }
        if let Some(url) = remote_url {
            query.push(("remoteUrl", url.to_owned()));
        }
        let path = "/api/spaces/detect";
        let response = self
            .http
            .get(format!("{}{path}", self.base))
            .query(&query)
            .send()
            .await
            .with_context(|| format!("could not reach fubbik at {}", self.base))?;
        if !response.status().is_success() {
            bail!("request to {path} failed with {}", response.status());
        }
        let body = response.bytes().await?;
        if body.is_empty() {
            return Ok(None);
        }
        Ok(Some(serde_json::from_slice(&body)?))
    }

    pub async fn delete_space(&self, id: &str) -> Result<serde_json::Value> {
        self.send_json(
            reqwest::Method::DELETE,
            &format!("/api/spaces/{id}"),
            serde_json::Value::Null,
        )
        .await
    }

    pub async fn list_tags(&self) -> Result<Vec<Tag>> {
        self.get_json("/api/tags", &[]).await
    }

    pub async fn create_tag(&self, name: &str, tag_type_id: Option<&str>) -> Result<Tag> {
        self.send_json(
            reqwest::Method::POST,
            "/api/tags",
            serde_json::json!({ "name": name, "tagTypeId": tag_type_id }),
        )
        .await
    }

    pub async fn update_tag(&self, id: &str, name: &str) -> Result<Tag> {
        self.send_json(
            reqwest::Method::PATCH,
            &format!("/api/tags/{id}"),
            serde_json::json!({ "name": name }),
        )
        .await
    }

    pub async fn delete_tag(&self, id: &str) -> Result<serde_json::Value> {
        self.send_json(
            reqwest::Method::DELETE,
            &format!("/api/tags/{id}"),
            serde_json::Value::Null,
        )
        .await
    }

    pub async fn create_connection(
        &self,
        source_id: &str,
        target_id: &str,
        relation: &str,
    ) -> Result<Connection> {
        self.send_json(
            reqwest::Method::POST,
            "/api/connections",
            serde_json::json!({
                "sourceId": source_id,
                "targetId": target_id,
                "relation": relation,
                "origin": "human",
            }),
        )
        .await
    }

    pub async fn delete_connection(&self, id: &str) -> Result<serde_json::Value> {
        self.send_json(
            reqwest::Method::DELETE,
            &format!("/api/connections/{id}"),
            serde_json::Value::Null,
        )
        .await
    }

    pub async fn list_requirements(
        &self,
        space_id: Option<&str>,
        status: Option<&str>,
        priority: Option<&str>,
    ) -> Result<Vec<Requirement>> {
        let mut query = Vec::new();
        if let Some(value) = space_id {
            query.push(("spaceId", value.to_owned()));
        }
        if let Some(value) = status {
            query.push(("status", value.to_owned()));
        }
        if let Some(value) = priority {
            query.push(("priority", value.to_owned()));
        }
        let response: RequirementListResponse = self.get_json("/api/requirements", &query).await?;
        Ok(response.requirements)
    }

    pub async fn create_requirement(
        &self,
        title: &str,
        steps: &[serde_json::Value],
        space_id: Option<&str>,
        priority: Option<&str>,
    ) -> Result<Requirement> {
        let response: RequirementMutationResponse = self
            .send_json(
                reqwest::Method::POST,
                "/api/requirements",
                serde_json::json!({
                    "title": title,
                    "steps": steps,
                    "spaceId": space_id,
                    "priority": priority,
                }),
            )
            .await?;
        Ok(response.requirement)
    }

    pub async fn update_requirement_status(&self, id: &str, status: &str) -> Result<Requirement> {
        self.send_json(
            reqwest::Method::PATCH,
            &format!("/api/requirements/{id}/status"),
            serde_json::json!({ "status": status }),
        )
        .await
    }

    pub async fn export_requirements(
        &self,
        format: &str,
        space_id: Option<&str>,
    ) -> Result<String> {
        let mut query = vec![("format", format.to_owned())];
        if let Some(value) = space_id {
            query.push(("spaceId", value.to_owned()));
        }
        self.get_text("/api/requirements/export", &query).await
    }

    pub async fn stats(&self) -> Result<serde_json::Value> {
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

    pub async fn list_documents(&self, space_id: Option<&str>) -> Result<Vec<serde_json::Value>> {
        let query = space_id
            .map(|value| vec![("spaceId", value.to_owned())])
            .unwrap_or_default();
        self.get_json("/api/documents", &query).await
    }

    pub async fn get_document(&self, id: &str) -> Result<serde_json::Value> {
        self.get_json(&format!("/api/documents/{id}"), &[]).await
    }

    pub async fn import_document(
        &self,
        source_path: &str,
        content: &str,
        space_id: Option<&str>,
    ) -> Result<serde_json::Value> {
        self.send_json(
            reqwest::Method::POST,
            "/api/documents/import",
            serde_json::json!({
                "sourcePath": source_path,
                "content": content,
                "spaceId": space_id,
            }),
        )
        .await
    }

    pub async fn import_source_docs(
        &self,
        space_id: &str,
        manifest: &fubbik_core::source_docs::SourceManifest,
    ) -> Result<serde_json::Value> {
        self.send_json(
            reqwest::Method::POST,
            "/api/documents/import-source",
            serde_json::json!({ "spaceId": space_id, "manifest": manifest }),
        )
        .await
    }

    pub async fn sync_document(
        &self,
        id: &str,
        content: &str,
        space_id: Option<&str>,
    ) -> Result<serde_json::Value> {
        self.send_json(
            reqwest::Method::POST,
            &format!("/api/documents/{id}/sync"),
            serde_json::json!({ "content": content, "spaceId": space_id }),
        )
        .await
    }

    pub async fn render_document(&self, id: &str) -> Result<serde_json::Value> {
        self.get_json(&format!("/api/documents/{id}/render"), &[])
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
