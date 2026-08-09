//! `plan_external_link` query surface, split out of `super` once the
//! combined `plan` repo file passed the ~900-line split threshold.
//! Re-exported at `crate::repo::plan::…` by the parent module so no
//! caller's import path changes.

use fubbik_core::error::AppResult;
use sqlx::PgPool;

use crate::timestamp::UtcTimestamp;

/// `camelCase` wire shape of a `plan_external_link` row.
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlanExternalLink {
    pub id: String,
    pub plan_id: String,
    pub system: String,
    pub url: String,
    pub label: Option<String>,
    pub order: i32,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub created_at: UtcTimestamp,
}

pub async fn list_links(
    pool: &PgPool,
    user_id: &str,
    plan_id: &str,
) -> AppResult<Vec<PlanExternalLink>> {
    let rows = sqlx::query_as!(
        PlanExternalLink,
        r#"SELECT id, plan_id, system, url, label, "order",
                  created_at AS "created_at: UtcTimestamp"
           FROM plan_external_link
           WHERE plan_id = $1
             AND EXISTS (SELECT 1 FROM plan p WHERE p.id = $1 AND p.user_id = $2)
           ORDER BY "order" ASC, created_at ASC"#,
        plan_id,
        user_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Inserts a link under a plan the caller owns — the `INSERT ... SELECT ...
/// FROM plan p WHERE ...` shape is the same ownership-guarded-insert
/// pattern as `fubbik_db::repo::workspace::add_space`: `None` back means
/// the plan isn't the caller's, mapped to 404 by the service layer.
pub async fn add_link(
    pool: &PgPool,
    user_id: &str,
    plan_id: &str,
    system: &str,
    url: &str,
    label: Option<&str>,
) -> AppResult<Option<PlanExternalLink>> {
    let id = crate::new_id();
    let link = sqlx::query_as!(
        PlanExternalLink,
        r#"INSERT INTO plan_external_link (id, plan_id, system, url, label)
           SELECT $1, p.id, $3, $4, $5
           FROM plan p
           WHERE p.id = $2 AND p.user_id = $6
           RETURNING id, plan_id, system, url, label, "order",
                     created_at AS "created_at: UtcTimestamp""#,
        id,
        plan_id,
        system,
        url,
        label,
        user_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(link)
}

/// Removes a link, scoped both to the named plan and to that plan's owner —
/// `false` covers "link never existed", "link belongs to a different plan",
/// and "plan isn't the caller's" alike.
pub async fn remove_link(
    pool: &PgPool,
    user_id: &str,
    plan_id: &str,
    link_id: &str,
) -> AppResult<bool> {
    let res = sqlx::query!(
        r#"DELETE FROM plan_external_link
           WHERE id = $1 AND plan_id = $2
             AND EXISTS (SELECT 1 FROM plan p WHERE p.id = $2 AND p.user_id = $3)"#,
        link_id,
        plan_id,
        user_id
    )
    .execute(pool)
    .await?;
    Ok(res.rows_affected() > 0)
}
