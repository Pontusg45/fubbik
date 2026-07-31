use axum::extract::State;
use axum::routing::{get, post};
use axum::{Json, Router};
use axum_extra::extract::CookieJar;
use axum_extra::extract::cookie::{Cookie, SameSite};
use chrono::Duration;
use fubbik_core::error::{AppError, AppResult};
use fubbik_db::repo::{session, user};

use super::password::{hash_password, verify_password};
use super::session::COOKIE_NAME;
use crate::AppState;

/// How long a session (and the cookie carrying its token) stays valid.
/// Named once so the DB-side session TTL and the cookie's `Max-Age` can
/// never drift apart — see `session_cookie` and both `session::create`
/// call sites below.
const SESSION_TTL_DAYS: i64 = 30;

#[derive(serde::Deserialize)]
pub struct SignUpBody {
    pub email: String,
    pub password: String,
    pub name: String,
}

#[derive(serde::Deserialize)]
pub struct SignInBody {
    pub email: String,
    pub password: String,
}

#[derive(serde::Serialize)]
pub struct UserResponse {
    pub id: String,
    pub email: String,
    pub name: String,
}

impl From<user::User> for UserResponse {
    fn from(u: user::User) -> Self {
        Self { id: u.id, email: u.email, name: u.name }
    }
}

fn session_cookie(token: String) -> Cookie<'static> {
    Cookie::build((COOKIE_NAME, token))
        .path("/")
        .http_only(true)
        .same_site(SameSite::Lax)
        .max_age(time::Duration::days(SESSION_TTL_DAYS))
        .build()
}

async fn sign_up(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(body): Json<SignUpBody>,
) -> AppResult<(CookieJar, Json<UserResponse>)> {
    if body.password.len() < 8 {
        return Err(AppError::Validation("password must be at least 8 characters".into()));
    }
    if user::find_by_email(&state.pool, &body.email).await?.is_some() {
        return Err(AppError::Conflict("email already registered".into()));
    }

    let hash = hash_password(&body.password)?;
    // The check above handles the common case, but it's a TOCTOU against
    // `user_email_unique`: two concurrent sign-ups for the same email can
    // both pass the check and race into the INSERT. Map that specific
    // failure back to the same clean 409 instead of letting it surface as
    // a generic 500.
    let u = match user::create(&state.pool, &body.email, &body.name, Some(&hash)).await {
        Ok(u) => u,
        Err(AppError::Database(sqlx::Error::Database(db_err))) if db_err.is_unique_violation() => {
            return Err(AppError::Conflict("email already registered".into()));
        }
        Err(e) => return Err(e),
    };
    let token = session::create(&state.pool, &u.id, Duration::days(SESSION_TTL_DAYS)).await?;

    Ok((jar.add(session_cookie(token)), Json(u.into())))
}

async fn sign_in(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(body): Json<SignInBody>,
) -> AppResult<(CookieJar, Json<UserResponse>)> {
    let u = user::find_by_email(&state.pool, &body.email)
        .await?
        .ok_or(AppError::Auth)?;

    let stored = u.password_hash.as_deref().ok_or(AppError::Auth)?;
    if !verify_password(&body.password, stored) {
        return Err(AppError::Auth);
    }

    let token = session::create(&state.pool, &u.id, Duration::days(SESSION_TTL_DAYS)).await?;
    Ok((jar.add(session_cookie(token)), Json(u.into())))
}

async fn sign_out(State(state): State<AppState>, jar: CookieJar) -> AppResult<CookieJar> {
    if let Some(c) = jar.get(COOKIE_NAME) {
        session::delete(&state.pool, c.value()).await?;
    }
    // The removal cookie's path must match the one the cookie was set with
    // (`/`, from `session_cookie`) or the browser will scope the deletion
    // to the request path instead and the original cookie will survive.
    let removal = Cookie::build(COOKIE_NAME).path("/").build();
    Ok(jar.remove(removal))
}

async fn get_session(current: super::CurrentUser) -> Json<UserResponse> {
    Json(current.0.into())
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/auth/sign-up/email", post(sign_up))
        .route("/api/auth/sign-in/email", post(sign_in))
        .route("/api/auth/sign-out", post(sign_out))
        .route("/api/auth/get-session", get(get_session))
}
