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
    let u = user::create(&state.pool, &body.email, &body.name, Some(&hash)).await?;
    let token = session::create(&state.pool, &u.id, Duration::days(30)).await?;

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

    let token = session::create(&state.pool, &u.id, Duration::days(30)).await?;
    Ok((jar.add(session_cookie(token)), Json(u.into())))
}

async fn sign_out(State(state): State<AppState>, jar: CookieJar) -> AppResult<CookieJar> {
    if let Some(c) = jar.get(COOKIE_NAME) {
        session::delete(&state.pool, c.value()).await?;
    }
    Ok(jar.remove(Cookie::from(COOKIE_NAME)))
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
