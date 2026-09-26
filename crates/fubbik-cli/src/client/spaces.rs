use super::*;

impl Client {
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

    pub async fn create_discovered_connection(
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
                "origin": "ai",
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
}
