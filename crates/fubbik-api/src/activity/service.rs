use fubbik_core::error::{AppError, AppResult};
use fubbik_db::repo::activity::{self, Activity, ListParams};
use fubbik_db::repo::space;
use sqlx::PgPool;

/// **Deliberate divergence from Node.** Node's `GET /activity`
/// (`packages/api/src/activity/routes.ts`) scopes the list by `user_id`
/// but applies `spaceId` as a bare equality filter with no check that the
/// space belongs to the caller — a `spaceId` for another user's space
/// simply matches none of the caller's own rows and comes back as a 200
/// with an empty array (rows are already `user_id`-scoped, so this can
/// never leak another user's data, unlike Task 5's `codebase_settings`
/// finding).
///
/// This port adds an explicit ownership pre-check here, the same shape as
/// `settings::service::get_all_codebase_settings`: a `spaceId` that
/// doesn't exist or isn't the caller's own now 404s instead of silently
/// returning `[]`. Combined with `activity::list`'s own `EXISTS` guard
/// (defense-in-depth against a TOCTOU race, same belt-and-suspenders shape
/// used throughout this crate), this is the "Node returns 200, Rust
/// returns 404" divergence called out in the task brief — required, not
/// discovered by accident.
pub async fn list(pool: &PgPool, user_id: &str, params: ListParams) -> AppResult<Vec<Activity>> {
    if let Some(space_id) = &params.space_id {
        space::find_by_id(pool, user_id, space_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Space".into()))?;
    }

    activity::list(pool, user_id, &params).await
}
