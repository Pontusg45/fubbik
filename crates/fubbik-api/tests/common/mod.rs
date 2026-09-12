#![allow(dead_code)]

use axum::Router;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use axum::response::Response;
use http_body_util::BodyExt;
use serde::de::DeserializeOwned;
use tower::ServiceExt;

#[derive(Debug, Clone)]
pub struct TestUser {
    cookie: String,
}

impl TestUser {
    pub fn cookie(&self) -> &str {
        &self.cookie
    }
}

#[derive(Clone)]
pub struct TestApp {
    router: Router,
}

impl TestApp {
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self::with_ai(pool, fubbik_ai::OllamaClient::new("http://127.0.0.1:1"))
    }

    pub fn with_ai(pool: sqlx::PgPool, ai: fubbik_ai::OllamaClient) -> Self {
        let state = fubbik_api::AppState {
            pool,
            implicit_dev_session: false,
            better_auth_secret: "test-secret".into(),
            ai,
            rate_limiter: Default::default(),
            background: Default::default(),
        };
        Self {
            router: fubbik_api::router(state),
        }
    }

    pub async fn signup(&self, email: &str, name: &str) -> TestUser {
        let response = self
            .request(
                None,
                Method::POST,
                "/api/auth/sign-up/email",
                Some(serde_json::json!({
                    "email": email,
                    "password": "hunter22",
                    "name": name,
                })),
            )
            .await;

        assert_eq!(
            response.status(),
            StatusCode::OK,
            "signup must succeed for {email}"
        );
        let cookie = response
            .headers()
            .get("set-cookie")
            .expect("signup should set a session cookie")
            .to_str()
            .expect("session cookie must be valid header text")
            .split(';')
            .next()
            .expect("session cookie must contain a name/value pair")
            .to_owned();

        TestUser { cookie }
    }

    pub async fn get(&self, user: &TestUser, path: &str) -> Response {
        self.request(Some(user), Method::GET, path, None).await
    }

    pub async fn post(&self, user: &TestUser, path: &str, body: serde_json::Value) -> Response {
        self.request(Some(user), Method::POST, path, Some(body))
            .await
    }

    pub async fn patch(&self, user: &TestUser, path: &str, body: serde_json::Value) -> Response {
        self.request(Some(user), Method::PATCH, path, Some(body))
            .await
    }

    pub async fn put(&self, user: &TestUser, path: &str, body: serde_json::Value) -> Response {
        self.request(Some(user), Method::PUT, path, Some(body))
            .await
    }

    pub async fn delete(&self, user: &TestUser, path: &str) -> Response {
        self.request(Some(user), Method::DELETE, path, None).await
    }

    pub async fn request(
        &self,
        user: Option<&TestUser>,
        method: Method,
        path: &str,
        body: Option<serde_json::Value>,
    ) -> Response {
        let has_body = body.is_some();
        let mut request = Request::builder().method(method).uri(path);
        if let Some(user) = user {
            request = request.header("cookie", user.cookie());
        }
        if has_body {
            request = request.header("content-type", "application/json");
        }

        self.router
            .clone()
            .oneshot(
                request
                    .body(body.map_or_else(Body::empty, |value| Body::from(value.to_string())))
                    .expect("test request must be valid"),
            )
            .await
            .expect("router must handle test request")
    }

    pub async fn json(response: Response) -> serde_json::Value {
        Self::json_as(response).await
    }

    pub async fn json_as<T: DeserializeOwned>(response: Response) -> T {
        let status = response.status();
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("response body must be readable")
            .to_bytes();
        serde_json::from_slice(&bytes).unwrap_or_else(|error| {
            panic!(
                "response with status {status} was not valid JSON: {error}; body={}",
                String::from_utf8_lossy(&bytes)
            )
        })
    }
}
