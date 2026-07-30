use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("database error: {0}")]
    Database(sqlx::Error),

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

/// Hand-written instead of `#[from]` so that `sqlx::Error::RowNotFound` maps
/// to `AppError::NotFound` instead of `AppError::Database`. A derived
/// `#[from]` treats every `sqlx::Error` the same, so a repository calling
/// `fetch_one()` on a query that legitimately returns zero rows would
/// surface a 500 instead of a 404. Phase 2 of this project has 48 route
/// domains built on top of this conversion — leaving that mistake to be
/// hand-rolled per call site (or worse, discovered in production) is not
/// worth the convenience `#[from]` buys here.
impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        match err {
            sqlx::Error::RowNotFound => AppError::NotFound("resource".to_string()),
            other => AppError::Database(other),
        }
    }
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
    use axum::body::to_bytes;
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

    #[test]
    fn external_maps_to_bad_gateway() {
        let err = AppError::External("upstream down".into());
        assert_eq!(err.into_response().status(), StatusCode::BAD_GATEWAY);
    }

    #[tokio::test]
    async fn database_error_does_not_leak_underlying_detail() {
        let err = AppError::Database(sqlx::Error::Protocol("SENTINEL_LEAK_CHECK".into()));
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body_str = std::str::from_utf8(&body).unwrap();

        assert_eq!(body_str, r#"{"message":"Internal server error"}"#);
        assert!(!body_str.contains("SENTINEL_LEAK_CHECK"));
    }

    #[test]
    fn row_not_found_converts_to_404_not_500() {
        let err: AppError = sqlx::Error::RowNotFound.into();
        assert_eq!(err.into_response().status(), StatusCode::NOT_FOUND);
    }
}
