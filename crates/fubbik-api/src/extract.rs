//! Extractors that wrap axum's built-ins so that request-parsing failures
//! come back as the same `{"message": ...}` JSON shape as every domain
//! error, instead of axum's default `text/plain` rejection body.
//!
//! Measured before this fix: `?sort=bogus` -> 400 text/plain, malformed
//! JSON body -> 400 text/plain, a missing required field -> 422
//! text/plain, missing content-type -> 415 text/plain. A client doing
//! `res.json()` broke on four of five error classes hitting this API.
//!
//! Status codes are preserved where they were already sensible:
//! `MissingJsonContentType` stays 415 (via `AppError::UnsupportedMediaType`),
//! and both a syntactically malformed body and an unparsable query string
//! stay 400. The one deliberate change is `JsonDataError` (a
//! present-but-invalid body, e.g. a missing required field): axum's
//! default for this is 422, but the service layer's own validation
//! failures (e.g. a blank title) are already 400 via `AppError::Validation`.
//! Routing both through `AppError::Validation` makes "the body I sent was
//! invalid" consistently 400, matching the service layer instead of
//! disagreeing with it.

use axum::extract::rejection::JsonRejection;
use axum::extract::{FromRequest, FromRequestParts, Request};
use axum::http::request::Parts;
use fubbik_core::error::AppError;
use serde::de::DeserializeOwned;

/// Drop-in replacement for `axum::extract::Json` whose rejection is
/// `AppError` instead of axum's `JsonRejection`.
pub struct Json<T>(pub T);

impl<T, S> FromRequest<S> for Json<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        match axum::Json::<T>::from_request(req, state).await {
            Ok(axum::Json(value)) => Ok(Json(value)),
            Err(rejection) => {
                let message = rejection.to_string();
                Err(match rejection {
                    // A wrong or absent content-type header is a distinct
                    // error class from "the body doesn't parse" — keep its
                    // existing, more specific status code.
                    JsonRejection::MissingJsonContentType(_) => {
                        AppError::UnsupportedMediaType(message)
                    }
                    _ => AppError::Validation(message),
                })
            }
        }
    }
}

/// Drop-in replacement for `axum::extract::Query` whose rejection is
/// `AppError` instead of axum's `QueryRejection`.
pub struct Query<T>(pub T);

impl<T, S> FromRequestParts<S> for Query<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        match axum::extract::Query::<T>::from_request_parts(parts, state).await {
            Ok(axum::extract::Query(value)) => Ok(Query(value)),
            Err(rejection) => Err(AppError::Validation(rejection.to_string())),
        }
    }
}
