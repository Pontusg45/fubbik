use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum_extra::extract::CookieJar;
use fubbik_core::error::AppError;
use fubbik_db::repo::{session, user};

use super::better_auth_cookie;
use crate::AppState;
use crate::error::ApiError;

pub const COOKIE_NAME: &str = "fubbik_session";

/// better-auth's own cookie name. HMAC-signed as `${rawToken}.${signature}`
/// — must be run through `better_auth_cookie::verify` before the raw token
/// inside it can be looked up.
pub const BETTER_AUTH_COOKIE_NAME: &str = "better-auth.session_token";

/// The `__Secure-` prefixed variant better-auth uses when `BETTER_AUTH_URL`
/// is `https://`. This is chosen by the URL scheme, not `NODE_ENV`, so the
/// server cannot predict which one a given browser will send — both names
/// are accepted unconditionally.
pub const BETTER_AUTH_SECURE_COOKIE_NAME: &str = "__Secure-better-auth.session_token";

/// Extractor yielding the authenticated user, or rejecting with 401.
pub struct CurrentUser(pub user::User);

impl FromRequestParts<AppState> for CurrentUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let jar = CookieJar::from_headers(&parts.headers);

        for name in [BETTER_AUTH_COOKIE_NAME, BETTER_AUTH_SECURE_COOKIE_NAME] {
            if let Some(cookie) = jar.get(name)
                && let Some(raw_token) =
                    better_auth_cookie::verify(cookie.value(), &state.better_auth_secret)
                && let Some(u) = session::find_valid(&state.pool, &raw_token).await?
            {
                return Ok(CurrentUser(u));
            }
        }

        if let Some(cookie) = jar.get(COOKIE_NAME)
            && let Some(u) = session::find_valid(&state.pool, cookie.value()).await?
        {
            return Ok(CurrentUser(u));
        }

        // Local-first escape hatch, mirroring FUBBIK_IMPLICIT_DEV_SESSION in
        // the TS server: fall back to the dev user rather than 401ing.
        //
        // Bootstraps the row lazily, per request, rather than once at
        // startup (see `user::ensure_implicit_dev_user`'s doc comment for
        // why this port chose that over mirroring Node's startup-time
        // `ensureImplicitDevUserRow` call): this is the only path any
        // `sqlx::test` router construction goes through — those build a
        // router directly with no separate startup hook — so a startup-time
        // call would leave the row missing in exactly the empty-database
        // case the bug shows up in. The extra cost lands on at most one
        // request per fresh database: `ensure_implicit_dev_user` does its
        // own find-by-id short-circuit before ever touching the insert, so
        // every request after the first is a single indexed SELECT, same as
        // the `find_by_email` call this replaced.
        if state.implicit_dev_session {
            let u = user::ensure_implicit_dev_user(&state.pool).await?;
            return Ok(CurrentUser(u));
        }

        Err(AppError::Auth.into())
    }
}

pub const DEV_EMAIL: &str = "dev@localhost";
