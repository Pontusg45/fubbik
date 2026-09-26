//! Health and aggregate-count HTTP responses shared across API and CLI.

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct HealthResponse {
    /// `ok | degraded`.
    pub status: String,
    /// `connected | disconnected`.
    pub db: String,
    pub age_available: bool,
}

/// Aggregate per-user counts backing `GET /api/stats`. Bare-object shape,
/// not an array and not the `{chunks,total,limit,offset}` envelope the
/// chunks *list* endpoint uses — matches Node's `getUserStats`
/// (`packages/api/src/stats/service.ts`) field-for-field. No fields beyond
/// what Node returns: this is a parity port, not a place to add "useful"
/// counts.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    pub chunks: i64,
    pub connections: i64,
    pub tags: i64,
}

#[cfg(test)]
mod tests {
    use super::{HealthResponse, Stats};

    #[test]
    fn system_responses_keep_the_published_wire_shapes() {
        // Given the current health and stats response payloads
        let health = serde_json::json!({"status":"ok","db":"connected","ageAvailable":true});
        let stats = serde_json::json!({"chunks":2,"connections":1,"tags":0});

        // When both payloads are decoded into shared contracts
        let decoded_health: HealthResponse = serde_json::from_value(health.clone()).unwrap();
        let decoded_stats: Stats = serde_json::from_value(stats.clone()).unwrap();

        // Then serialization preserves their public JSON fields
        assert_eq!(decoded_health.status, "ok");
        assert_eq!(decoded_stats.chunks, 2);
        assert_eq!(serde_json::to_value(decoded_health).unwrap(), health);
        assert_eq!(serde_json::to_value(decoded_stats).unwrap(), stats);
    }
}
