//! Supervision for non-request work started by one API server instance.

use std::future::Future;
use std::time::Duration;

use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;

/// Owns the cancellation boundary and task set for one server instance.
///
/// Clones refer to the same underlying cancellation token and tracker. A new
/// value creates an independent runtime, which keeps tests and multiple server
/// instances from cancelling one another.
#[derive(Clone, Debug)]
pub struct BackgroundRuntime {
    cancellation: CancellationToken,
    tracker: TaskTracker,
}

impl BackgroundRuntime {
    pub fn new() -> Self {
        Self {
            cancellation: CancellationToken::new(),
            tracker: TaskTracker::new(),
        }
    }

    pub fn cancellation_token(&self) -> CancellationToken {
        self.cancellation.clone()
    }

    pub fn spawn<F>(&self, future: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        drop(self.tracker.spawn(future));
    }

    /// Stops recurring jobs and gives in-flight work a bounded drain window.
    pub async fn shutdown(&self, timeout: Duration) {
        self.cancellation.cancel();
        self.tracker.close();
        if tokio::time::timeout(timeout, self.tracker.wait())
            .await
            .is_err()
        {
            tracing::warn!(
                ?timeout,
                "background tasks did not drain before shutdown deadline"
            );
        }
    }
}

impl Default for BackgroundRuntime {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::BackgroundRuntime;

    #[tokio::test]
    async fn shutdown_is_scoped_to_one_runtime() {
        // Given
        let first = BackgroundRuntime::new();
        let second = BackgroundRuntime::new();
        let first_token = first.cancellation_token();
        let second_token = second.cancellation_token();

        first.spawn({
            let token = first_token.clone();
            async move { token.cancelled().await }
        });

        // When
        first.shutdown(Duration::from_secs(1)).await;

        // Then
        assert!(first_token.is_cancelled());
        assert!(!second_token.is_cancelled());
        second.shutdown(Duration::from_secs(1)).await;
    }
}
