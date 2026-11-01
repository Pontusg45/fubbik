use anyhow::{Context, Result, bail};
use reqwest::header::{COOKIE, HeaderMap, HeaderValue};

use super::Client;

impl Client {
    pub fn new(base: impl Into<String>) -> Self {
        Self::with_token(base, None).expect("http client builds")
    }

    pub fn authenticated(base: impl Into<String>) -> Result<Self> {
        let base = base.into();
        let token = crate::auth::token_for(&base)?;
        Self::with_token(base, token.as_deref())
    }

    pub fn with_token(base: impl Into<String>, token: Option<&str>) -> Result<Self> {
        let mut headers = HeaderMap::new();
        if let Some(token) = token {
            headers.insert(
                COOKIE,
                HeaderValue::from_str(&format!("fubbik_session={token}"))?,
            );
        }
        Ok(Self {
            base: base.into().trim_end_matches('/').to_owned(),
            http: reqwest::Client::builder()
                .cookie_store(true)
                .default_headers(headers)
                .redirect(reqwest::redirect::Policy::none())
                .connect_timeout(std::time::Duration::from_secs(3))
                .timeout(std::time::Duration::from_secs(60))
                .build()?,
        })
    }

    pub fn base_url(&self) -> &str {
        &self.base
    }

    pub async fn sign_in(&self, email: &str, password: &str) -> Result<super::SignInResponse> {
        self.send_json(
            reqwest::Method::POST,
            "/api/auth/sign-in/email",
            serde_json::json!({"email": email, "password": password}),
        )
        .await
    }

    pub async fn sign_out(&self) -> Result<()> {
        let _: serde_json::Value = self
            .send_json(
                reqwest::Method::POST,
                "/api/auth/sign-out",
                serde_json::Value::Null,
            )
            .await?;
        Ok(())
    }

    pub async fn current_session(&self) -> Result<Option<serde_json::Value>> {
        self.get_json("/api/auth/get-session", &[]).await
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
            .with_context(|| format!("could not reach fubbik at {} (is the fubbik desktop app, or `fubbik serve`, running?)", self.base))?;
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
