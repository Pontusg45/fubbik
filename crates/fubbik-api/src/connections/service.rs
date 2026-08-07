use fubbik_core::error::{AppError, AppResult};
use fubbik_db::repo::chunk;
use fubbik_db::repo::connection::{self, Connection};
use sqlx::PgPool;

use super::dto::CreateConnectionBody;

/// Mirrors Node's `createConnection` (`packages/api/src/connections/service.ts:11-34`).
///
/// The happy path is a single round trip: `connection::create`'s `INSERT
/// ... SELECT ... WHERE s.user_id = $5 AND t.user_id = $5` is the actual
/// authorization check, verified in SQL rather than pre-checked here and
/// then trusted. Only on the *failure* path (`Ok(None)`) do we spend two
/// extra `chunk::find_by_id` calls, purely to match Node's distinct
/// "Source chunk"/"Target chunk" 404 messages — Node pays that cost on
/// every request (two `getChunkById` calls before ever attempting the
/// insert); this only pays it when something was actually rejected.
pub async fn create(
    pool: &PgPool,
    user_id: &str,
    body: CreateConnectionBody,
) -> AppResult<Connection> {
    if body.source_id == body.target_id {
        return Err(AppError::Validation(
            "Cannot connect a chunk to itself".into(),
        ));
    }

    let origin = body.origin.map(|o| o.as_str()).unwrap_or("human");
    let review_status = if origin == "ai" { "draft" } else { "approved" };
    let id = fubbik_db::new_id();

    let result = connection::create(
        pool,
        &id,
        user_id,
        &body.source_id,
        &body.target_id,
        &body.relation,
        origin,
        review_status,
    )
    .await;

    match result {
        Ok(Some(conn)) => Ok(conn),
        Ok(None) => {
            // The ownership guard rejected the pair. Figure out which
            // side, to match Node's distinct resource names.
            if chunk::find_by_id(pool, user_id, &body.source_id)
                .await?
                .is_none()
            {
                return Err(AppError::NotFound("Source chunk".into()));
            }
            if chunk::find_by_id(pool, user_id, &body.target_id)
                .await?
                .is_none()
            {
                return Err(AppError::NotFound("Target chunk".into()));
            }
            // Both chunks resolve for this user, yet the guarded insert
            // still matched no rows — should not happen outside a race
            // with a concurrent delete of one of them between the two
            // lookups above. No Node-specific message for this case;
            // fall back to a generic not-found.
            Err(AppError::NotFound("Connection".into()))
        }
        Err(AppError::Database(sqlx::Error::Database(db_err))) if db_err.is_unique_violation() => {
            Err(AppError::Conflict("connection already exists".into()))
        }
        Err(AppError::Database(sqlx::Error::Database(db_err)))
            if db_err.is_foreign_key_violation() =>
        {
            Err(AppError::Validation(format!(
                "Invalid relation \"{}\"",
                body.relation
            )))
        }
        Err(e) => Err(e),
    }
}

/// Mirrors Node's `deleteConnection` (`packages/api/src/connections/service.ts:36-51`):
/// 404 `{resource: "Connection"}` both when the id doesn't exist at all and
/// when it exists but neither `source` nor `target` resolves for
/// `user_id` — `connection::delete` folds both cases into a single guarded
/// `DELETE`, so there is nothing left to distinguish here.
pub async fn delete(pool: &PgPool, user_id: &str, id: &str) -> AppResult<()> {
    if connection::delete(pool, user_id, id).await? {
        Ok(())
    } else {
        Err(AppError::NotFound("Connection".into()))
    }
}
