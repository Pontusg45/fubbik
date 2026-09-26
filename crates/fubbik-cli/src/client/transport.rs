use anyhow::{Context, Result, bail};

use super::Client;

impl Client {
    pub fn new(base: impl Into<String>) -> Self {
        Self {
            base: base.into().trim_end_matches('/').to_owned(),
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

    async fn response(
        &self,
        request: reqwest::RequestBuilder,
        method: &reqwest::Method,
        path: &str,
    ) -> Result<reqwest::Response> {
        let response = request
            .send()
            .await
            .with_context(|| format!("could not reach fubbik at {}", self.base))?;
        if response.status().is_success() {
            return Ok(response);
        }

        let status = response.status();
        let request_id = response
            .headers()
            .get("x-request-id")
            .and_then(|header| header.to_str().ok())
            .map(str::to_owned);
        let body = response.text().await.unwrap_or_default();
        let detail = serde_json::from_str::<serde_json::Value>(&body)
            .ok()
            .and_then(|value| {
                value["message"]
                    .as_str()
                    .or_else(|| value["error"].as_str())
                    .map(str::to_owned)
            })
            .unwrap_or_else(|| body.trim().chars().take(500).collect());
        let request_id = request_id
            .map(|id| format!(" (request ID {id})"))
            .unwrap_or_default();
        if detail.is_empty() {
            bail!("{method} {path} failed with {status}{request_id}");
        }
        bail!("{method} {path} failed with {status}{request_id}: {detail}");
    }

    pub(super) async fn get_json<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, String)],
    ) -> Result<T> {
        let response = self
            .response(
                self.http.get(format!("{}{path}", self.base)).query(query),
                &reqwest::Method::GET,
                path,
            )
            .await?;
        Ok(response.json().await?)
    }

    pub(super) async fn send_json<T: serde::de::DeserializeOwned>(
        &self,
        method: reqwest::Method,
        path: &str,
        body: serde_json::Value,
    ) -> Result<T> {
        let response = self
            .response(
                self.http
                    .request(method.clone(), format!("{}{path}", self.base))
                    .json(&body),
                &method,
                path,
            )
            .await?;
        Ok(response.json().await?)
    }

    pub(super) async fn get_text(&self, path: &str, query: &[(&str, String)]) -> Result<String> {
        let response = self
            .response(
                self.http.get(format!("{}{path}", self.base)).query(query),
                &reqwest::Method::GET,
                path,
            )
            .await?;
        Ok(response.text().await?)
    }

    pub(super) async fn send_empty(&self, method: reqwest::Method, path: &str) -> Result<()> {
        self.response(
            self.http
                .request(method.clone(), format!("{}{path}", self.base)),
            &method,
            path,
        )
        .await?;
        Ok(())
    }
}
