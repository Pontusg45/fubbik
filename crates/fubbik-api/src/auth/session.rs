use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum_extra::extract::CookieJar;
use fubbik_core::error::AppError;
use fubbik_db::repo::{session, user};

use crate::AppState;

pub const COOKIE_NAME: &str = "fubbik_session";

/// Extractor yielding the authenticated user, or rejecting with 401.
pub struct CurrentUser(pub user::User);

impl FromRequestParts<AppState> for CurrentUser {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let jar = CookieJar::from_headers(&parts.headers);

        if let Some(cookie) = jar.get(COOKIE_NAME)
            && let Some(u) = session::find_valid(&state.pool, cookie.value()).await?
        {
            return Ok(CurrentUser(u));
        }

        // Local-first escape hatch, mirroring FUBBIK_IMPLICIT_DEV_SESSION in
        // the TS server: fall back to the dev user rather than 401ing.
        if state.implicit_dev_session
            && let Some(u) = user::find_by_email(&state.pool, DEV_EMAIL).await?
        {
            return Ok(CurrentUser(u));
        }

        Err(AppError::Auth)
    }
}

pub const DEV_EMAIL: &str = "dev@fubbik.local";
