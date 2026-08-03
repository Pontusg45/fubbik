//! Owns the HTTP concern for `fubbik_core::error::AppError`.
//!
//! `AppError` itself carries no `IntoResponse` impl — that would force
//! `fubbik-core` (and everything under it, including `fubbik-db`) to depend
//! on axum. Rust's orphan rule also forbids implementing the *foreign*
//! `IntoResponse` trait for the *foreign* `AppError` type here in
//! `fubbik-api`, so `ApiError` exists as a local newtype purely to give
//! `IntoResponse` a type it's allowed to touch. `From<AppError>` keeps `?`
//! working in handlers exactly as before.

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use fubbik_core::error::AppError;

pub type ApiResult<T> = Result<T, ApiError>;

pub struct ApiError(pub AppError);

impl From<AppError> for ApiError {
    fn from(err: AppError) -> Self {
        Self(err)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let err = self.0;
        let status = match &err {
            AppError::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::NotFound(_) => StatusCode::NOT_FOUND,
            AppError::Auth => StatusCode::UNAUTHORIZED,
            AppError::Validation(_) => StatusCode::BAD_REQUEST,
            AppError::UnsupportedMediaType(_) => StatusCode::UNSUPPORTED_MEDIA_TYPE,
            AppError::Conflict(_) => StatusCode::CONFLICT,
            AppError::External(_) => StatusCode::BAD_GATEWAY,
        };

        // Internal errors are logged in full but never leak detail to the
        // client. Everything else is safe to surface verbatim.
        let message = match &err {
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
    use super::ApiError;
    use axum::body::to_bytes;
    use axum::http::StatusCode;
    use axum::response::IntoResponse;
    use fubbik_core::error::AppError;

    #[test]
    fn maps_variants_to_status_codes() {
        let cases = [
            (AppError::NotFound("chunk".into()), StatusCode::NOT_FOUND),
            (AppError::Auth, StatusCode::UNAUTHORIZED),
            (AppError::Validation("bad".into()), StatusCode::BAD_REQUEST),
            (AppError::Conflict("dupe".into()), StatusCode::CONFLICT),
        ];
        for (err, expected) in cases {
            assert_eq!(ApiError::from(err).into_response().status(), expected);
        }
    }

    #[test]
    fn external_maps_to_bad_gateway() {
        let err = AppError::External("upstream down".into());
        assert_eq!(
            ApiError::from(err).into_response().status(),
            StatusCode::BAD_GATEWAY
        );
    }

    #[test]
    fn unsupported_media_type_maps_to_415() {
        let err = AppError::UnsupportedMediaType("expected application/json".into());
        assert_eq!(
            ApiError::from(err).into_response().status(),
            StatusCode::UNSUPPORTED_MEDIA_TYPE
        );
    }

    #[tokio::test]
    async fn database_error_does_not_leak_underlying_detail() {
        let err = AppError::Database(sqlx::Error::Protocol("SENTINEL_LEAK_CHECK".into()));
        let response = ApiError::from(err).into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body_str = std::str::from_utf8(&body).unwrap();

        assert_eq!(body_str, r#"{"message":"Internal server error"}"#);
        assert!(!body_str.contains("SENTINEL_LEAK_CHECK"));
    }

    #[test]
    fn row_not_found_converts_to_404_not_500() {
        let err: AppError = sqlx::Error::RowNotFound.into();
        assert_eq!(
            ApiError::from(err).into_response().status(),
            StatusCode::NOT_FOUND
        );
    }
}
