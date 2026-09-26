use super::*;

impl Client {
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

    pub async fn list_recent_chunks(
        &self,
        days: u32,
        space_id: Option<&str>,
    ) -> Result<RecentChunks> {
        let mut query = vec![
            ("after", days.to_string()),
            ("sort", "updated".to_string()),
            ("limit", "100".to_string()),
        ];
        if let Some(space_id) = space_id {
            query.push(("spaceId", space_id.to_string()));
        }
        let res: RecapChunkListResponse = self.get_json("/api/chunks", &query).await?;
        Ok(RecentChunks {
            chunks: res.chunks,
            total: res.total,
        })
    }

    pub async fn list_prompt_chunks(&self, search: Option<&str>) -> Result<Vec<Chunk>> {
        let mut query = vec![("tags", "prompt".to_string()), ("limit", "50".to_string())];
        if let Some(search) = search {
            query.push(("search", search.to_string()));
            query.push(("limit", "1".to_string()));
        }
        let res: ChunkListResponse = self.get_json("/api/chunks", &query).await?;
        Ok(res.chunks)
    }

    pub async fn list_chunk_page(&self, space_id: Option<&str>, offset: u32) -> Result<ChunkPage> {
        let mut query = vec![
            ("limit", "100".to_string()),
            ("offset", offset.to_string()),
            ("sort", "alpha".to_string()),
        ];
        if let Some(space_id) = space_id {
            query.push(("spaceId", space_id.to_owned()));
        }
        let response: ChunkListResponse = self.get_json("/api/chunks", &query).await?;
        Ok(ChunkPage {
            chunks: response.chunks,
            total: response.total,
        })
    }

    /// Collect every page of chunk summaries, preserving the API's alpha order.
    pub async fn list_all_chunks(&self, space_id: Option<&str>) -> Result<Vec<Chunk>> {
        let mut chunks = Vec::new();
        loop {
            let offset = u32::try_from(chunks.len()).context("too many chunks to paginate")?;
            let page = self.list_chunk_page(space_id, offset).await?;
            if page.chunks.is_empty() {
                if i64::try_from(chunks.len())? < page.total {
                    bail!(
                        "chunk list ended before the reported total of {}",
                        page.total
                    );
                }
                break;
            }
            chunks.extend(page.chunks);
            if i64::try_from(chunks.len())? >= page.total {
                break;
            }
        }
        Ok(chunks)
    }

    pub async fn get_chunk_detail(&self, id: &str) -> Result<serde_json::Value> {
        self.get_json(&format!("/api/chunks/{id}"), &[]).await
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
        self.create_chunk_with_update_tag(title, content, chunk_type, tags, spaces, None)
            .await
    }

    pub async fn create_discovered_chunk(
        &self,
        title: &str,
        content: &str,
        chunk_type: &str,
        tags: &[String],
        space_id: &str,
    ) -> Result<Chunk> {
        self.send_json(
            reqwest::Method::POST,
            "/api/chunks",
            serde_json::json!({
                "title": title,
                "content": content,
                "type": chunk_type,
                "tags": tags,
                "spaceIds": [space_id],
                "origin": "ai",
            }),
        )
        .await
    }

    pub async fn create_chunk_with_update_tag(
        &self,
        title: &str,
        content: &str,
        chunk_type: &str,
        tags: &[String],
        spaces: &[String],
        update_tag: Option<&str>,
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
                "updateTag": update_tag,
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

    pub async fn set_applies_to(&self, id: &str, patterns: &[String]) -> Result<serde_json::Value> {
        let body = patterns
            .iter()
            .map(|pattern| serde_json::json!({"pattern": pattern}))
            .collect::<Vec<_>>();
        self.send_json(
            reqwest::Method::PUT,
            &format!("/api/chunks/{id}/applies-to"),
            body.into(),
        )
        .await
    }
}
