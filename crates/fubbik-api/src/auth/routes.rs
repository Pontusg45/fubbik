use axum::extract::State;
use axum::routing::{get, post};
use axum::{Json, Router};
use axum_extra::extract::CookieJar;
use axum_extra::extract::cookie::{Cookie, SameSite};
use chrono::Duration;
use fubbik_core::error::AppError;
use fubbik_db::repo::{session, user};

use super::password::{hash_password, verify_better_auth_password, verify_password};
use super::session::{BETTER_AUTH_COOKIE_NAME, BETTER_AUTH_SECURE_COOKIE_NAME, COOKIE_NAME};
use crate::AppState;
use crate::error::ApiResult;
use crate::extract::Json as ReqJson;

/// How long a session (and the cookie carrying its token) stays valid.
/// Named once so the DB-side session TTL and the cookie's `Max-Age` can
/// never drift apart — see `session_cookie` and both `session::create`
/// call sites below.
const SESSION_TTL_DAYS: i64 = 30;
const MIN_PASSWORD_LENGTH: usize = 8;
const MAX_PASSWORD_LENGTH: usize = 128;

/// Matches the practical ASCII email shape Better Auth validates with Zod.
/// Keeping this check at the HTTP boundary prevents malformed identities
/// from reaching the database while avoiding a second email-validation
/// policy in the repository layer, which is also used by fixtures and the
/// implicit development user.
fn is_valid_email(email: &str) -> bool {
    if !email.is_ascii() || email.starts_with('.') || email.contains("..") {
        return false;
    }

    let Some((local, domain)) = email.split_once('@') else {
        return false;
    };
    if local.is_empty() || domain.contains('@') {
        return false;
    }
    if !local
        .bytes()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'\'' | b'+' | b'-' | b'.'))
        || !local
            .as_bytes()
            .last()
            .is_some_and(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'+' | b'-'))
    {
        return false;
    }

    let mut labels = domain.split('.').peekable();
    let mut label_count = 0;
    while let Some(label) = labels.next() {
        label_count += 1;
        let is_last = labels.peek().is_none();
        if label.is_empty()
            || !label.as_bytes()[0].is_ascii_alphanumeric()
            || if is_last {
                label.len() < 2 || !label.bytes().all(|c| c.is_ascii_alphabetic())
            } else {
                !label
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || c == b'-')
            }
        {
            return false;
        }
    }
    label_count >= 2
}

/// JavaScript's `string.length` counts UTF-16 code units. Better Auth's
/// password limits therefore do too; reproducing that definition avoids
/// accepting a value here that the retired server would reject.
fn password_length(password: &str) -> usize {
    password.encode_utf16().count()
}

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
#[serde(rename_all = "camelCase")]
pub struct UserResponse {
    pub id: String,
    pub email: String,
    pub name: String,
    pub email_verified: bool,
    pub image: Option<String>,
    pub created_at: fubbik_db::timestamp::UtcTimestamp,
    pub updated_at: fubbik_db::timestamp::UtcTimestamp,
}

impl From<user::User> for UserResponse {
    fn from(u: user::User) -> Self {
        Self {
            id: u.id,
            email: u.email,
            name: u.name,
            email_verified: u.email_verified,
            image: u.image,
            created_at: u.created_at,
            updated_at: u.updated_at,
        }
    }
}

#[derive(serde::Serialize)]
pub struct SignUpResponse {
    pub token: String,
    pub user: UserResponse,
}

#[derive(serde::Serialize)]
pub struct SignInResponse {
    pub redirect: bool,
    pub token: String,
    pub url: Option<String>,
    pub user: UserResponse,
}

#[derive(serde::Serialize)]
pub struct SignOutResponse {
    pub success: bool,
}

#[derive(serde::Serialize)]
pub struct SessionResponse {
    pub session: session::Session,
    pub user: UserResponse,
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
    ReqJson(body): ReqJson<SignUpBody>,
) -> ApiResult<(CookieJar, Json<SignUpResponse>)> {
    if !is_valid_email(&body.email) {
        return Err(AppError::Validation("invalid email".into()).into());
    }
    let password_length = password_length(&body.password);
    if password_length < MIN_PASSWORD_LENGTH {
        return Err(AppError::Validation("password must be at least 8 characters".into()).into());
    }
    if password_length > MAX_PASSWORD_LENGTH {
        return Err(AppError::Validation("password must be at most 128 characters".into()).into());
    }
    let email = body.email.to_ascii_lowercase();
    if user::find_by_email(&state.pool, &email).await?.is_some() {
        return Err(AppError::Conflict("email already registered".into()).into());
    }

    let hash = hash_password(&body.password)?;
    // The check above handles the common case, but it's a TOCTOU against
    // `user_email_unique`: two concurrent sign-ups for the same email can
    // both pass the check and race into the INSERT. Map that specific
    // failure back to the same clean 409 instead of letting it surface as
    // a generic 500.
    let u = match user::create(&state.pool, &email, &body.name, Some(&hash)).await {
        Ok(u) => u,
        Err(AppError::Database(sqlx::Error::Database(db_err))) if db_err.is_unique_violation() => {
            return Err(AppError::Conflict("email already registered".into()).into());
        }
        Err(e) => return Err(e.into()),
    };
    let token = session::create(&state.pool, &u.id, Duration::days(SESSION_TTL_DAYS)).await?;

    Ok((
        jar.add(session_cookie(token.clone())),
        Json(SignUpResponse {
            token,
            user: u.into(),
        }),
    ))
}

async fn sign_in(
    State(state): State<AppState>,
    jar: CookieJar,
    ReqJson(body): ReqJson<SignInBody>,
) -> ApiResult<(CookieJar, Json<SignInResponse>)> {
    if !is_valid_email(&body.email) {
        return Err(AppError::Validation("invalid email".into()).into());
    }
    let email = body.email.to_ascii_lowercase();
    let u = user::find_by_email(&state.pool, &email)
        .await?
        .ok_or(AppError::Auth)?;

    let valid = if let Some(stored) = u.password_hash.as_deref() {
        verify_password(&body.password, stored)
    } else if let Some(legacy) = user::credential_password(&state.pool, &u.id).await? {
        if verify_better_auth_password(&body.password, &legacy) {
            let upgraded = hash_password(&body.password)?;
            user::upgrade_credential_password(&state.pool, &u.id, &upgraded).await?;
            true
        } else {
            false
        }
    } else {
        false
    };
    if !valid {
        return Err(AppError::Auth.into());
    }

    let token = session::create(&state.pool, &u.id, Duration::days(SESSION_TTL_DAYS)).await?;
    Ok((
        jar.add(session_cookie(token.clone())),
        Json(SignInResponse {
            redirect: false,
            token,
            url: None,
            user: u.into(),
        }),
    ))
}

async fn sign_out(
    State(state): State<AppState>,
    jar: CookieJar,
) -> ApiResult<(CookieJar, Json<SignOutResponse>)> {
    let mut jar = jar;
    for name in [
        COOKIE_NAME,
        BETTER_AUTH_COOKIE_NAME,
        BETTER_AUTH_SECURE_COOKIE_NAME,
    ] {
        if let Some(cookie) = jar.get(name) {
            let raw = if name == COOKIE_NAME {
                Some(cookie.value().to_owned())
            } else {
                super::better_auth_cookie::verify(cookie.value(), &state.better_auth_secret)
            };
            if let Some(raw) = raw {
                session::delete(&state.pool, &raw).await?;
            }
        }
        jar = jar.remove(
            Cookie::build(name)
                .path("/")
                .secure(name == BETTER_AUTH_SECURE_COOKIE_NAME)
                .build(),
        );
    }
    Ok((jar, Json(SignOutResponse { success: true })))
}

async fn get_session(
    State(state): State<AppState>,
    jar: CookieJar,
) -> ApiResult<Json<Option<SessionResponse>>> {
    let mut raw_token = None;
    for name in [BETTER_AUTH_COOKIE_NAME, BETTER_AUTH_SECURE_COOKIE_NAME] {
        if let Some(cookie) = jar.get(name)
            && let Some(raw) =
                super::better_auth_cookie::verify(cookie.value(), &state.better_auth_secret)
        {
            raw_token = Some(raw);
            break;
        }
    }
    if raw_token.is_none() {
        raw_token = jar.get(COOKIE_NAME).map(|cookie| cookie.value().to_owned());
    }
    let Some(raw_token) = raw_token else {
        return Ok(Json(None));
    };
    let Some(record) = session::find_valid_session(&state.pool, &raw_token).await? else {
        return Ok(Json(None));
    };
    let Some(owner) = user::find_by_id(&state.pool, &record.user_id).await? else {
        return Ok(Json(None));
    };
    Ok(Json(Some(SessionResponse {
        session: record,
        user: owner.into(),
    })))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/auth/sign-up/email", post(sign_up))
        .route("/api/auth/sign-in/email", post(sign_in))
        .route("/api/auth/sign-out", post(sign_out))
        .route("/api/auth/get-session", get(get_session))
}
