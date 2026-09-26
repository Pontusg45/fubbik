use std::process::Output;

use fubbik_api::{AppState, background::BackgroundRuntime};
use fubbik_cli::client::Client;
use sqlx::PgPool;

pub struct TestApplication {
    pub pool: PgPool,
    pub client: Client,
    pub base_url: String,
    server: tokio::task::JoinHandle<()>,
}

impl TestApplication {
    pub async fn start(pool: PgPool) -> Self {
        let state = AppState {
            pool: pool.clone(),
            implicit_dev_session: true,
            better_auth_secret: "test-secret-at-least-32-characters-123".into(),
            ai: fubbik_ai::OllamaClient::new("http://127.0.0.1:1"),
            rate_limiter: Default::default(),
            background: BackgroundRuntime::new(),
        };
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let base_url = format!("http://127.0.0.1:{port}");
        let server = tokio::spawn(async move {
            axum::serve(listener, fubbik_api::router(state))
                .await
                .unwrap();
        });
        Self {
            pool,
            client: Client::new(&base_url),
            base_url,
            server,
        }
    }

    pub async fn run_cli(&self, args: &[&str]) -> serde_json::Value {
        let output = self.run_cli_raw(args).await;
        assert!(
            output.status.success(),
            "fubbik {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).unwrap()
    }

    pub async fn run_cli_raw(&self, args: &[&str]) -> Output {
        self.run_cli_with_flags(&["--json"], args).await
    }

    pub async fn run_cli_with_flags(&self, flags: &[&str], args: &[&str]) -> Output {
        tokio::process::Command::new(env!("CARGO_BIN_EXE_fubbik"))
            .args(flags)
            .args(["--url", &self.base_url])
            .args(args)
            .output()
            .await
            .expect("fubbik binary runs")
    }
}

impl Drop for TestApplication {
    fn drop(&mut self) {
        self.server.abort();
    }
}
