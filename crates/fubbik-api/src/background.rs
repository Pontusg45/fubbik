//! Supervision for non-request work started by API handlers and schedulers.

use std::future::Future;
use std::sync::LazyLock;
use std::time::Duration;

use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;

struct BackgroundRuntime {
    cancellation: CancellationToken,
    tracker: TaskTracker,
}

static RUNTIME: LazyLock<BackgroundRuntime> = LazyLock::new(|| BackgroundRuntime {
    cancellation: CancellationToken::new(),
    tracker: TaskTracker::new(),
});

pub fn cancellation_token() -> CancellationToken {
    RUNTIME.cancellation.clone()
}

pub fn spawn<F>(future: F)
where
    F: Future<Output = ()> + Send + 'static,
{
    drop(RUNTIME.tracker.spawn(future));
}

/// Stops recurring jobs and gives in-flight work a bounded drain window.
pub async fn shutdown(timeout: Duration) {
    RUNTIME.cancellation.cancel();
    RUNTIME.tracker.close();
    if tokio::time::timeout(timeout, RUNTIME.tracker.wait())
        .await
        .is_err()
    {
        tracing::warn!(
            ?timeout,
            "background tasks did not drain before shutdown deadline"
        );
    }
}
