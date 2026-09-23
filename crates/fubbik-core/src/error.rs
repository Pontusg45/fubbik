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

    #[error("unsupported media type: {0}")]
    UnsupportedMediaType(String),

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

#[cfg(test)]
mod tests {
    use super::AppError;

    #[test]
    fn row_not_found_converts_to_404_not_500() {
        // Given the inline inputs and test fixtures.
        // When
        let err: AppError = sqlx::Error::RowNotFound.into();
        // Then
        assert!(matches!(err, AppError::NotFound(_)));
    }
}
