//! Explains a 404 from a feature-gated command (`plan`, `matrix`).
//!
//! The server leaves a switched-off group's routes unmounted, so the CLI sees
//! a bare 404. This asks `/api/capabilities` whether the group is off and, if
//! so, says that instead. Servers without the endpoint, or an unreachable
//! one, leave the original error untouched.

use std::time::Duration;

/// `Some(enabled keys)` from `/api/capabilities`, or `None` when unknown.
pub async fn enabled_features(base: &str) -> Option<Vec<String>> {
    let response = reqwest::Client::new()
        .get(format!("{}/api/capabilities", base.trim_end_matches('/')))
        .timeout(Duration::from_secs(2))
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    let body: serde_json::Value = response.json().await.ok()?;
    Some(
        body.get("features")?
            .as_array()?
            .iter()
            .filter_map(|feature| feature.as_str().map(str::to_owned))
            .collect(),
    )
}

/// Whether `error` is the transport error for a 404 response.
fn is_not_found(error: &anyhow::Error) -> bool {
    error.to_string().contains(" failed with 404")
}

/// The message shown when `feature` is switched off.
pub fn disabled_message(feature: &str) -> String {
    format!("feature \"{feature}\" is disabled on this server")
}

/// If `error` is a 404 and the server reports `feature` as disabled, print
/// `error: feature "<key>" is disabled on this server` and exit 1. Otherwise
/// return `error` unchanged.
pub async fn explain_not_found(base: &str, feature: &str, error: anyhow::Error) -> anyhow::Error {
    if is_not_found(&error)
        && let Some(enabled) = enabled_features(base).await
        && !enabled.iter().any(|key| key == feature)
    {
        eprintln!("error: {}", disabled_message(feature));
        std::process::exit(1);
    }
    error
}
