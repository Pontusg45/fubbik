use fubbik_core::error::AppResult;
use sqlx::PgPool;

use crate::timestamp::UtcTimestamp;

/// `camelCase` serialisation matches every other wire type in this crate —
/// see the note on `chunk::Chunk` for why that's mandatory, not cosmetic.
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TagType {
    pub id: String,
    pub name: String,
    pub color: String,
    pub icon: Option<String>,
    pub user_id: String,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub created_at: UtcTimestamp,
}

/// Inserts a new tag type. `color` is `NOT NULL DEFAULT '#8b5cf6'` in the
/// schema; when the caller passes `None` the `color` column is left out of
/// the `INSERT` entirely so Postgres applies its own default, rather than
/// this function re-stating the hex literal (the schema stays the single
/// source of truth for it).
pub async fn create(
    pool: &PgPool,
    user_id: &str,
    name: &str,
    color: Option<&str>,
    icon: Option<&str>,
) -> AppResult<TagType> {
    let id = crate::new_id();
    let t = match color {
        Some(color) => {
            sqlx::query_as!(
                TagType,
                r#"INSERT INTO tag_type (id, name, color, icon, user_id)
                   VALUES ($1, $2, $3, $4, $5)
                   RETURNING id, name, color, icon, user_id,
                             created_at AS "created_at: UtcTimestamp""#,
                id,
                name,
                color,
                icon,
                user_id
            )
            .fetch_one(pool)
            .await?
        }
        None => {
            sqlx::query_as!(
                TagType,
                r#"INSERT INTO tag_type (id, name, icon, user_id)
                   VALUES ($1, $2, $3, $4)
                   RETURNING id, name, color, icon, user_id,
                             created_at AS "created_at: UtcTimestamp""#,
                id,
                name,
                icon,
                user_id
            )
            .fetch_one(pool)
            .await?
        }
    };
    Ok(t)
}

/// Lists a user's tag types. Every query in this module filters `user_id`
/// in SQL so cross-user access is impossible by construction.
///
/// `, id ASC` is a tiebreaker over `created_at`, which is not unique — see
/// `chunk::list`'s equivalent comment.
pub async fn list(pool: &PgPool, user_id: &str) -> AppResult<Vec<TagType>> {
    let rows = sqlx::query_as!(
        TagType,
        r#"SELECT id, name, color, icon, user_id,
                  created_at AS "created_at: UtcTimestamp"
           FROM tag_type WHERE user_id = $1 ORDER BY created_at ASC, id ASC"#,
        user_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Applies only the fields present in the patch. `COALESCE` keeps unset
/// columns untouched, matching `chunk::update`.
///
/// `icon` is tri-state: `None` (key absent from the request) leaves the
/// column untouched, `Some(None)` (explicit JSON `null`) clears it to
/// `NULL`, `Some(Some(v))` sets it. Plain `COALESCE($n, icon)` cannot express
/// this — both "absent" and "explicit null" would flatten to a bound SQL
/// `NULL`, and `COALESCE` treats any `NULL` parameter as "leave unchanged",
/// silently no-opping an explicit clear. Instead this uses the same
/// `CASE WHEN $set::bool THEN $val::text ELSE icon END` idiom as
/// `tag::update`'s handling of `tag_type_id` and `space::update`'s handling
/// of `description`/`remote_url`: a side-channel boolean parameter carries
/// "was this key present at all", decoupled from whether its value is
/// `NULL`.
pub async fn update(
    pool: &PgPool,
    user_id: &str,
    id: &str,
    name: Option<&str>,
    color: Option<&str>,
    icon: Option<Option<&str>>,
) -> AppResult<Option<TagType>> {
    let (icon_set, icon_val) = match icon {
        Some(v) => (true, v),
        None => (false, None),
    };
    let t = sqlx::query_as!(
        TagType,
        r#"UPDATE tag_type SET
             name = COALESCE($3, name),
             color = COALESCE($4, color),
             icon = CASE WHEN $5::bool THEN $6::text ELSE icon END
           WHERE id = $1 AND user_id = $2
           RETURNING id, name, color, icon, user_id,
                     created_at AS "created_at: UtcTimestamp""#,
        id,
        user_id,
        name,
        color,
        icon_set,
        icon_val
    )
    .fetch_optional(pool)
    .await?;
    Ok(t)
}

/// Deletes a tag type. Any `tag` rows referencing it via `tag_type_id` are
/// nulled out by the database's own `ON DELETE SET NULL` foreign key
/// (`migrations/0001_init.sql`, `tag_tag_type_id_tag_type_id_fk`) — this
/// function deliberately does not touch the `tag` table itself, matching
/// Node (`packages/db/src/repository/tag-type.ts:28-36`), which never
/// restricts or cascades in application code either.
pub async fn delete(pool: &PgPool, user_id: &str, id: &str) -> AppResult<bool> {
    let res = sqlx::query!(
        "DELETE FROM tag_type WHERE id = $1 AND user_id = $2",
        id,
        user_id
    )
    .execute(pool)
    .await?;
    Ok(res.rows_affected() > 0)
}
