//! `GET /api/capabilities` — which `fubbik.toml` feature groups this server
//! mounted. The web app reads it to hide navigation for disabled groups.

use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};

use crate::AppState;

#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct CapabilitiesResponse {
    /// Enabled feature keys, in a fixed order: `plans`, `requirements`,
    /// `matrices`, `coordination`, `learning_paths`, `overlays`, `ai`.
    pub features: Vec<String>,
}

/// **Deliberately unauthenticated**: it reveals only which route groups
/// exist, which a 404 would reveal anyway, and the web app needs it before
/// sign-in to draw the nav.
#[utoipa::path(get, path = "/api/capabilities",
    responses((status = 200, body = CapabilitiesResponse)))]
pub async fn capabilities(State(state): State<AppState>) -> Json<CapabilitiesResponse> {
    Json(CapabilitiesResponse {
        features: state
            .features
            .enabled_keys()
            .into_iter()
            .map(String::from)
            .collect(),
    })
}

pub fn router() -> Router<AppState> {
    Router::new().route("/api/capabilities", get(capabilities))
}
