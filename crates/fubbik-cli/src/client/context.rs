use super::*;

impl Client {
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

    pub async fn context_for_plan(
        &self,
        plan_id: &str,
        max_tokens: usize,
        format: &str,
    ) -> Result<serde_json::Value> {
        self.get_json(
            "/api/context/for-plan",
            &[
                ("planId", plan_id.to_owned()),
                ("maxTokens", max_tokens.to_string()),
                ("format", format.to_owned()),
            ],
        )
        .await
    }

    pub async fn context_for_files(
        &self,
        paths: &[String],
        space: Option<&str>,
        max_tokens: usize,
        format: &str,
    ) -> Result<serde_json::Value> {
        let mut query = vec![
            ("paths", paths.join(",")),
            ("maxTokens", max_tokens.to_string()),
            ("format", format.to_owned()),
        ];
        if let Some(value) = space {
            query.push(("spaceId", value.to_owned()));
        }
        self.get_json("/api/context/for-files", &query).await
    }

    pub async fn create_context_snapshot(
        &self,
        plan_id: Option<&str>,
        task_id: Option<&str>,
        concept: Option<&str>,
        file_paths: &[String],
        space_id: Option<&str>,
        max_tokens: usize,
    ) -> Result<serde_json::Value> {
        self.send_json(
            reqwest::Method::POST,
            "/api/context/snapshot",
            serde_json::json!({
                "planId": plan_id,
                "taskId": task_id,
                "concept": concept,
                "filePaths": file_paths,
                "spaceId": space_id,
                "maxTokens": max_tokens,
            }),
        )
        .await
    }

    pub async fn get_context_snapshot(&self, id: &str) -> Result<serde_json::Value> {
        self.get_json(&format!("/api/context/snapshot/{id}"), &[])
            .await
    }

    pub async fn list_context_snapshots(&self) -> Result<Vec<serde_json::Value>> {
        self.get_json("/api/context/snapshots", &[]).await
    }

    pub async fn delete_context_snapshot(&self, id: &str) -> Result<()> {
        self.send_empty(
            reqwest::Method::DELETE,
            &format!("/api/context/snapshot/{id}"),
        )
        .await
    }

    pub async fn context_about(
        &self,
        concept: &str,
        space: Option<&str>,
        max_tokens: usize,
        format: &str,
    ) -> Result<serde_json::Value> {
        let mut query = vec![
            ("q", concept.to_string()),
            ("maxTokens", max_tokens.to_string()),
            ("format", format.to_string()),
        ];
        if let Some(value) = space {
            query.push(("spaceId", value.to_string()));
        }
        self.get_json("/api/context/about", &query).await
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

    pub async fn generate_instructions(
        &self,
        space_id: &str,
        format: &str,
    ) -> Result<serde_json::Value> {
        self.get_json(
            &format!("/api/spaces/{space_id}/generate-instructions"),
            &[("format", format.to_owned())],
        )
        .await
    }
}
