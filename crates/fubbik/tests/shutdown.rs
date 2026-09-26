#![cfg(unix)]

use std::time::Duration;

use sqlx::{ConnectOptions, PgPool};
use tokio::time::{sleep, timeout};

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn sigterm_drains_an_active_request_before_exit(pool: PgPool) {
    // Given a Rust server and an exclusive lock that holds a chunk-list read open
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let base_url = format!("http://127.0.0.1:{port}");
    let mut command = tokio::process::Command::new(env!("CARGO_BIN_EXE_fubbik"));
    command
        .args(["serve", "--port", &port.to_string()])
        .env(
            "DATABASE_URL",
            pool.connect_options().to_url_lossy().as_str(),
        )
        .env(
            "BETTER_AUTH_SECRET",
            "shutdown-test-secret-at-least-32-chars",
        )
        .env("FUBBIK_IMPLICIT_DEV_SESSION", "true")
        .kill_on_drop(true);
    let mut server = command.spawn().unwrap();
    let client = reqwest::Client::new();
    timeout(Duration::from_secs(20), async {
        loop {
            if client
                .get(format!("{base_url}/api/health"))
                .send()
                .await
                .is_ok()
            {
                break;
            }
            sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("server becomes healthy");

    let mut lock = pool.begin().await.unwrap();
    sqlx::query("LOCK TABLE chunk IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *lock)
        .await
        .unwrap();
    let active_request = tokio::spawn({
        let client = client.clone();
        let base_url = base_url.clone();
        async move { client.get(format!("{base_url}/api/chunks")).send().await }
    });
    timeout(Duration::from_secs(10), async {
        loop {
            let blocked: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM pg_stat_activity WHERE datname = current_database() \
                 AND wait_event_type = 'Lock' AND query LIKE 'SELECT id, title,%'",
            )
            .fetch_one(&pool)
            .await
            .unwrap();
            if blocked > 0 {
                break;
            }
            sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("chunk-list request blocks on the lock");

    // When SIGTERM arrives while that request is still in progress
    let signal = std::process::Command::new("kill")
        .args(["-TERM", &server.id().unwrap().to_string()])
        .status()
        .unwrap();
    assert!(signal.success());
    sleep(Duration::from_millis(150)).await;
    assert!(
        server.try_wait().unwrap().is_none(),
        "server must drain the request"
    );
    lock.rollback().await.unwrap();

    // Then the request succeeds and the process exits cleanly
    let response = timeout(Duration::from_secs(20), active_request)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(response.status().is_success());
    let exit = timeout(Duration::from_secs(20), server.wait())
        .await
        .unwrap()
        .unwrap();
    assert!(exit.success(), "server exited with {exit}");
}
