use anyhow::{Context, Result, bail};

/// Mirrors the API's camelCase output. The CLI deserialises the wire format,
/// not the database row, so this must match `fubbik_db::repo::chunk::Chunk`'s
/// serde representation rather than its field names.
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Chunk {
    pub id: String,
    pub title: String,
    #[serde(rename = "type")]
    pub chunk_type: String,
    pub content: String,
    pub updated_at: String,
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
}
