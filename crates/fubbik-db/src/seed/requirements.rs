use anyhow::{Result, anyhow};
use serde::Deserialize;
use sqlx::{Postgres, Transaction};
use std::collections::BTreeMap;

use super::spaces::DEV_USER_ID;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RequirementFixture {
    name: String,
    title: String,
    description: String,
    steps: serde_json::Value,
    priority: String,
    use_case: Option<String>,
    #[serde(default)]
    chunk_names: Vec<String>,
}

pub async fn seed_requirements(
    tx: &mut Transaction<'_, Postgres>,
    use_case_ids: &BTreeMap<String, String>,
    chunk_ids: &BTreeMap<String, String>,
) -> Result<BTreeMap<String, String>> {
    let fixtures: Vec<RequirementFixture> =
        serde_json::from_str(include_str!("requirements-catalog.json"))?;
    let space_id: String =
        sqlx::query_scalar("SELECT id FROM space WHERE user_id = $1 AND name = 'fubbik'")
            .bind(DEV_USER_ID)
            .fetch_one(&mut **tx)
            .await?;
    let mut ids = BTreeMap::new();
    for fixture in fixtures {
        let existing: Option<String> =
            sqlx::query_scalar("SELECT id FROM requirement WHERE user_id = $1 AND title = $2")
                .bind(DEV_USER_ID)
                .bind(&fixture.title)
                .fetch_optional(&mut **tx)
                .await?;
        let id = existing.unwrap_or_else(|| format!("seed-requirement-{}", fixture.name));
        let use_case_id = fixture
            .use_case
            .as_ref()
            .map(|name| {
                use_case_ids
                    .get(name)
                    .ok_or_else(|| anyhow!("Unknown use case: {name}"))
            })
            .transpose()?;
        sqlx::query(
            "INSERT INTO requirement (id, title, description, steps, priority, status, user_id, space_id, use_case_id) \
             VALUES ($1, $2, $3, $4, $5, 'untested', $6, $7, $8) ON CONFLICT (id) DO UPDATE SET \
             title = EXCLUDED.title, description = EXCLUDED.description, \
             steps = EXCLUDED.steps, priority = EXCLUDED.priority, use_case_id = EXCLUDED.use_case_id, \
             updated_at = now()",
        )
        .bind(&id)
        .bind(&fixture.title)
        .bind(&fixture.description)
        .bind(&fixture.steps)
        .bind(&fixture.priority)
        .bind(DEV_USER_ID)
        .bind(&space_id)
        .bind(use_case_id)
        .execute(&mut **tx)
        .await?;
        for name in &fixture.chunk_names {
            let chunk_id = chunk_ids.get(name).ok_or_else(|| {
                anyhow!(
                    "Requirement {} references unknown chunk {name}",
                    fixture.name
                )
            })?;
            sqlx::query(
                "INSERT INTO requirement_chunk (requirement_id, chunk_id) \
                 VALUES ($1, $2) ON CONFLICT DO NOTHING",
            )
            .bind(&id)
            .bind(chunk_id)
            .execute(&mut **tx)
            .await?;
        }
        ids.insert(fixture.name, id);
    }
    Ok(ids)
}
