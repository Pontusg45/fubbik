use anyhow::{Context, Result, bail};
use reqwest::Method;
use serde_json::Value;

pub(crate) struct ApiClient {
    base: String,
    http: reqwest::Client,
}

impl ApiClient {
    pub(crate) fn new(base: impl Into<String>) -> Self {
        Self {
            base: base.into().trim_end_matches('/').to_string(),
            http: reqwest::Client::new(),
        }
    }

    pub(crate) async fn get(&self, path: &str, query: &[(&str, String)]) -> Result<Value> {
        let response = self
            .http
            .get(format!("{}{path}", self.base))
            .query(query)
            .send()
            .await
            .with_context(|| format!("could not reach fubbik at {} (is the fubbik desktop app, or `fubbik serve`, running?)", self.base))?;
        decode(response, path).await
    }

    /// Enabled feature keys from `GET /api/capabilities`, or `None` when the
    /// server is unreachable, slow, or too old to have the endpoint.
    pub(crate) async fn capabilities(&self) -> Option<Vec<String>> {
        let response = self
            .http
            .get(format!("{}/api/capabilities", self.base))
            .timeout(std::time::Duration::from_secs(2))
            .send()
            .await
            .ok()?;
        if !response.status().is_success() {
            return None;
        }
        let body: Value = response.json().await.ok()?;
        let features = body.get("features")?.as_array()?;
        Some(
            features
                .iter()
                .filter_map(|feature| feature.as_str().map(str::to_string))
                .collect(),
        )
    }

    pub(crate) async fn send(&self, method: Method, path: &str, body: Value) -> Result<Value> {
        let response = self
            .http
            .request(method, format!("{}{path}", self.base))
            .json(&body)
            .send()
            .await
            .with_context(|| format!("could not reach fubbik at {} (is the fubbik desktop app, or `fubbik serve`, running?)", self.base))?;
        decode(response, path).await
    }
}

async fn decode(response: reqwest::Response, path: &str) -> Result<Value> {
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        bail!("API {status} for {path}: {body}");
    }
    Ok(response.json().await?)
}
