use anyhow::Result;
use sqlx::{Postgres, Transaction};
use std::collections::BTreeMap;

use super::spaces::DEV_USER_ID;

pub async fn seed_use_cases(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<BTreeMap<String, String>> {
    let space_id: String =
        sqlx::query_scalar("SELECT id FROM space WHERE user_id = $1 AND name = 'fubbik'")
            .bind(DEV_USER_ID)
            .fetch_one(&mut **tx)
            .await?;
    let fixtures = [
        (
            "onboarding",
            "New team member onboarding",
            "A dev joins the fubbik team and wants to understand the architecture in a day.",
        ),
        (
            "decisions",
            "Capture decisions near the code",
            "An engineer makes an architecture decision and wants a persistent, queryable record linked to the relevant files.",
        ),
    ];
    let mut ids = BTreeMap::new();
    for (name, title, description) in fixtures {
        let existing: Option<String> =
            sqlx::query_scalar("SELECT id FROM use_case WHERE user_id = $1 AND name = $2")
                .bind(DEV_USER_ID)
                .bind(title)
                .fetch_optional(&mut **tx)
                .await?;
        let id = existing.unwrap_or_else(|| format!("seed-use-case-{name}"));
        sqlx::query(
            "INSERT INTO use_case (id, name, description, space_id, user_id) \
             VALUES ($1, $2, $3, $4, $5) ON CONFLICT (id) DO UPDATE SET \
             name = EXCLUDED.name, description = EXCLUDED.description, \
             space_id = EXCLUDED.space_id, updated_at = now()",
        )
        .bind(&id)
        .bind(title)
        .bind(description)
        .bind(&space_id)
        .bind(DEV_USER_ID)
        .execute(&mut **tx)
        .await?;
        ids.insert(name.to_string(), id);
    }
    Ok(ids)
}
