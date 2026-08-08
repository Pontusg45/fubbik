use fubbik_core::error::AppResult;
use sqlx::PgPool;

use crate::timestamp::UtcTimestamp;

/// `notification.type` is `text NOT NULL` with no check constraint and no
/// enum anywhere in the Node source (`packages/db/src/schema/notification.ts`
/// just leaves a comment listing example values). The Phase 2b plan's
/// general instruction to model constrained value sets as enum-typed DTO
/// fields (like `connections::dto::Origin`) does NOT apply here — the
/// captured contract found no constraint to enforce, so this stays a plain
/// `String` end to end. Adding validation Node doesn't have would be a
/// silent, undocumented divergence.
///
/// `camelCase` serialisation matches every other wire type in this crate —
/// see the note on `chunk::Chunk` for why that's mandatory, not cosmetic.
/// `notification_type` is renamed to the reserved SQL/JSON word `type` for
/// the same reason `chunk::Chunk::chunk_type` is.
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Notification {
    pub id: String,
    pub user_id: String,
    #[serde(rename = "type")]
    pub notification_type: String,
    pub title: String,
    pub message: String,
    pub link_to: Option<String>,
    pub read: bool,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub created_at: UtcTimestamp,
}

/// Lists a user's notifications, optionally restricted to unread ones, with
/// Node's default cap of 50 rows when the caller doesn't specify a limit
/// (`packages/api/src/notifications/routes.ts`'s `t.Optional(t.Numeric())`
/// query param, defaulted at the service layer).
///
/// `ORDER BY created_at DESC, id ASC` is a total order: `created_at` is not
/// unique — batch inserts (seed data, a burst of staleness notifications
/// fired in the same request) routinely share the exact same timestamp, and
/// an `ORDER BY` over tied rows with no deterministic tiebreaker is a
/// query-plan artifact, not a stable order — see `chunk::list`'s equivalent
/// comment. `id ASC` matches the tiebreaker direction used everywhere else
/// in this crate (`chunk::list`'s `Sort::Newest`, `tag::list`, `space::list`),
/// so ties resolve consistently across domains.
pub async fn list(
    pool: &PgPool,
    user_id: &str,
    unread_only: bool,
    limit: i64,
) -> AppResult<Vec<Notification>> {
    let rows = sqlx::query_as!(
        Notification,
        r#"SELECT id, user_id, type AS notification_type, title, message, link_to, read,
                  created_at AS "created_at: UtcTimestamp"
           FROM notification
           WHERE user_id = $1 AND ($2::bool = false OR read = false)
           ORDER BY created_at DESC, id ASC
           LIMIT $3"#,
        user_id,
        unread_only,
        limit
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Counts a user's unread notifications. Scoped by `user_id` in SQL, same
/// as every other query in this module.
pub async fn count_unread(pool: &PgPool, user_id: &str) -> AppResult<i64> {
    let count = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "count!" FROM notification WHERE user_id = $1 AND read = false"#,
        user_id
    )
    .fetch_one(pool)
    .await?;
    Ok(count)
}

/// Marks a single notification read. Returns `None` — not an error — when
/// the id doesn't exist or belongs to another user, matching Node's
/// `markAsRead`, which returns `updated ?? null` from the same
/// `WHERE id = $1 AND user_id = $2` filter; the service layer turns `None`
/// into a 404.
pub async fn mark_read(pool: &PgPool, user_id: &str, id: &str) -> AppResult<Option<Notification>> {
    let n = sqlx::query_as!(
        Notification,
        r#"UPDATE notification SET read = true
           WHERE id = $1 AND user_id = $2
           RETURNING id, user_id, type AS notification_type, title, message, link_to, read,
                     created_at AS "created_at: UtcTimestamp""#,
        id,
        user_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(n)
}

/// Marks every one of a user's unread notifications read in one statement.
/// This is a bulk write with no per-row id check, so the `user_id = $1`
/// filter in the `WHERE` clause is the *only* thing standing between this
/// and marking another user's notifications read with no error raised —
/// see `tests/notification.rs::mark_all_read_does_not_touch_another_users_rows`.
pub async fn mark_all_read(pool: &PgPool, user_id: &str) -> AppResult<()> {
    sqlx::query!(
        "UPDATE notification SET read = true WHERE user_id = $1 AND read = false",
        user_id
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// Deletes a notification. Returns whether a row was actually removed so
/// the service layer can distinguish "gone" from "never existed / not
/// yours" and answer both with 404, matching Node.
pub async fn delete(pool: &PgPool, user_id: &str, id: &str) -> AppResult<bool> {
    let res = sqlx::query!(
        "DELETE FROM notification WHERE id = $1 AND user_id = $2",
        id,
        user_id
    )
    .execute(pool)
    .await?;
    Ok(res.rows_affected() > 0)
}
