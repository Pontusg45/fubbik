use std::time::Duration;

use sqlx::PgPool;

pub fn spawn_projection_worker(pool: PgPool, background: crate::background::BackgroundRuntime) {
    let cancellation = background.cancellation_token();
    background.spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                _ = cancellation.cancelled() => break,
                _ = interval.tick() => {
                    match fubbik_db::repo::projection::process_batch(&pool, 50).await {
                        Ok(batch) if batch.completed > 0 || batch.failed > 0 => {
                            tracing::info!(completed = batch.completed, failed = batch.failed, "projection outbox batch processed");
                        }
                        Ok(_) => {}
                        Err(error) => tracing::warn!(%error, "projection outbox batch failed"),
                    }
                }
            }
        }
    });
}
