//! `StepValidationError`'s HTTP shape doesn't fit the generic `ApiError`
//! (`crate::error::ApiError`), which only ever emits `{"message": ...}` —
//! Node's step-validation failure is `{"message": "Invalid steps",
//! "errors": [...]}` (`packages/api/src/index.ts:197-199`). No other
//! domain in this port has needed a structured (non-message-only) error
//! body, so rather than growing `fubbik_core::error::AppError` with a
//! variant only this domain uses, this module carries a small local
//! wrapper — the same "orphan-rule newtype implementing `IntoResponse`"
//! shape `crate::error::ApiError` itself uses, one level up.

use axum::Json;
use axum::response::{IntoResponse, Response};
use fubbik_core::error::AppError;

use crate::error::ApiError;

pub type RequirementResult<T> = Result<T, RequirementError>;

pub enum RequirementError {
    App(AppError),
    /// Pre-serialized `errors` array — the element shape differs between
    /// `POST /requirements`/`PATCH /requirements/{id}` (`{step, error}`,
    /// `validator::StepError`) and `POST /requirements/batch`
    /// (`{index, step, error}`, `dto::BatchStepError`), so this carries
    /// whichever shape the caller already serialized rather than picking
    /// one concrete type.
    StepValidation(serde_json::Value),
}

impl From<AppError> for RequirementError {
    fn from(err: AppError) -> Self {
        Self::App(err)
    }
}

impl IntoResponse for RequirementError {
    fn into_response(self) -> Response {
        match self {
            RequirementError::App(err) => ApiError::from(err).into_response(),
            RequirementError::StepValidation(errors) => (
                axum::http::StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "message": "Invalid steps", "errors": errors })),
            )
                .into_response(),
        }
    }
}
