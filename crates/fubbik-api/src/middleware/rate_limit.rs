//! Ports `packages/api/src/middleware/rate-limit.ts`.
//!
//! Deliberately not an axum layer. Node applies this at two call sites —
//! enrich and semantic search — and nothing else asks for it; a tower layer
//! would be more machinery than either needs and would have to reach for
//! the session user id from inside the request extensions anyway.
//!
//! One divergence from Node: expired windows are evicted during `check`
//! rather than by a 5-minute `setInterval`. The map is keyed per user per
//! endpoint and every entry is reclaimed the next time any key is looked
//! up, so it stays bounded without a background task.
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy)]
pub struct RateLimitDecision {
    pub allowed: bool,
    /// Seconds until the current window resets. Node rounds up
    /// (`Math.ceil`), so a caller denied 0.4s before reset is told 1, never
    /// 0 — a 0 would invite an immediate retry that is still denied.
    pub retry_after_secs: i64,
}

struct Window {
    count: u32,
    reset_at: Instant,
}

#[derive(Clone, Default)]
pub struct RateLimiter {
    windows: Arc<Mutex<HashMap<String, Window>>>,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn check(&self, key: &str, max: u32, window: Duration) -> RateLimitDecision {
        let now = Instant::now();
        let mut windows = self.windows.lock().expect("rate limiter mutex poisoned");

        // Opportunistic eviction, standing in for Node's sweep timer. Runs
        // before the entry lookup so a key whose own window has expired is
        // recreated fresh rather than resurrected with its old count.
        windows.retain(|_, w| w.reset_at > now);

        let entry = windows.entry(key.to_string()).or_insert_with(|| Window {
            count: 0,
            reset_at: now + window,
        });
        entry.count += 1;

        let allowed = entry.count <= max;
        let remaining = entry.reset_at.saturating_duration_since(now);
        RateLimitDecision {
            allowed,
            retry_after_secs: remaining.as_secs_f64().ceil() as i64,
        }
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.windows.lock().unwrap().len()
    }

    #[cfg(test)]
    fn contains(&self, key: &str) -> bool {
        self.windows.lock().unwrap().contains_key(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn allows_up_to_the_limit_then_denies() {
        let limiter = RateLimiter::new();
        for i in 0..10 {
            assert!(
                limiter.check("k", 10, Duration::from_secs(60)).allowed,
                "call {i}"
            );
        }
        assert!(!limiter.check("k", 10, Duration::from_secs(60)).allowed);
    }

    #[test]
    fn keys_do_not_share_a_window() {
        let limiter = RateLimiter::new();
        for _ in 0..10 {
            limiter.check("a", 10, Duration::from_secs(60));
        }
        assert!(
            limiter.check("b", 10, Duration::from_secs(60)).allowed,
            "one user exhausting their budget must not block another"
        );
    }

    #[test]
    fn the_window_resets_once_it_expires() {
        let limiter = RateLimiter::new();
        // A zero-length window is already expired on the next lookup, which
        // exercises the reset branch without a sleep.
        assert!(limiter.check("k", 1, Duration::from_secs(0)).allowed);
        assert!(limiter.check("k", 1, Duration::from_secs(0)).allowed);
    }

    #[test]
    fn a_denial_reports_seconds_until_reset() {
        let limiter = RateLimiter::new();
        limiter.check("k", 1, Duration::from_secs(60));
        let decision = limiter.check("k", 1, Duration::from_secs(60));
        assert!(!decision.allowed);
        assert!(
            decision.retry_after_secs > 0 && decision.retry_after_secs <= 60,
            "got {}",
            decision.retry_after_secs
        );
    }

    /// Node sweeps expired windows on a 5-minute timer
    /// (`middleware/rate-limit.ts:22-31`). This port evicts on lookup
    /// instead, so the map cannot grow without bound and no background task
    /// is needed.
    #[test]
    fn expired_entries_are_evicted_on_lookup() {
        let limiter = RateLimiter::new();
        limiter.check("gone", 1, Duration::from_secs(0));
        limiter.check("kept", 1, Duration::from_secs(60));
        assert_eq!(limiter.len(), 1);
        assert!(limiter.contains("kept"));
    }
}
