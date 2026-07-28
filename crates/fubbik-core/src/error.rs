use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("{0} not found")]
    NotFound(String),

    #[error("unauthorized")]
    Auth,

    #[error("validation failed: {0}")]
    Validation(String),

    #[error("conflict: {0}")]
    Conflict(String),

    #[error("external service error: {0}")]
    External(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = match &self {
            AppError::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::NotFound(_) => StatusCode::NOT_FOUND,
            AppError::Auth => StatusCode::UNAUTHORIZED,
            AppError::Validation(_) => StatusCode::BAD_REQUEST,
            AppError::Conflict(_) => StatusCode::CONFLICT,
            AppError::External(_) => StatusCode::BAD_GATEWAY,
        };

        // Internal errors are logged in full but never leak detail to the
        // client. Everything else is safe to surface verbatim.
        let message = match &self {
            AppError::Database(e) => {
                tracing::error!("database error: {e:?}");
                "Internal server error".to_string()
            }
            other => other.to_string(),
        };

        (status, Json(serde_json::json!({ "message": message }))).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::AppError;
    use axum::http::StatusCode;
    use axum::response::IntoResponse;

    #[test]
    fn maps_variants_to_status_codes() {
        let cases = [
            (AppError::NotFound("chunk".into()), StatusCode::NOT_FOUND),
            (AppError::Auth, StatusCode::UNAUTHORIZED),
            (AppError::Validation("bad".into()), StatusCode::BAD_REQUEST),
            (AppError::Conflict("dupe".into()), StatusCode::CONFLICT),
        ];
        for (err, expected) in cases {
            assert_eq!(err.into_response().status(), expected);
        }
    }
}
