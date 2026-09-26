use anyhow::{Result, anyhow};
use serde::Deserialize;
use sqlx::{Postgres, Transaction};
use std::collections::BTreeMap;

use super::spaces::DEV_USER_ID;

#[derive(Deserialize)]
struct ChunkFixture {
    name: String,
    title: String,
    #[serde(rename = "type")]
    kind: Option<String>,
    content: Option<String>,
    summary: Option<String>,
    rationale: Option<String>,
    consequences: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
}

pub async fn seed_chunks(tx: &mut Transaction<'_, Postgres>) -> Result<BTreeMap<String, String>> {
    let fixtures: Vec<ChunkFixture> = serde_json::from_str(include_str!("chunks-catalog.json"))?;
    let space_id: String =
        sqlx::query_scalar("SELECT id FROM space WHERE user_id = $1 AND name = 'fubbik'")
            .bind(DEV_USER_ID)
            .fetch_one(&mut **tx)
            .await?;
    let mut ids = BTreeMap::new();
    for fixture in fixtures {
        if ids.contains_key(&fixture.name) {
            return Err(anyhow!("Duplicate chunk fixture name: {}", fixture.name));
        }
        let existing: Option<String> =
            sqlx::query_scalar("SELECT id FROM chunk WHERE user_id = $1 AND title = $2")
                .bind(DEV_USER_ID)
                .bind(&fixture.title)
                .fetch_optional(&mut **tx)
                .await?;
        let id = existing.unwrap_or_else(|| format!("seed-chunk-{}", fixture.name));
        sqlx::query(
            "INSERT INTO chunk (id, title, type, content, summary, rationale, consequences, user_id) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8) ON CONFLICT (id) DO UPDATE SET \
             title = EXCLUDED.title, type = EXCLUDED.type, content = EXCLUDED.content, \
             summary = EXCLUDED.summary, rationale = EXCLUDED.rationale, \
             consequences = EXCLUDED.consequences, updated_at = now()",
        )
        .bind(&id)
        .bind(&fixture.title)
        .bind(fixture.kind.as_deref().unwrap_or("note"))
        .bind(fixture.content.as_deref().unwrap_or(""))
        .bind(&fixture.summary)
        .bind(&fixture.rationale)
        .bind(&fixture.consequences)
        .bind(DEV_USER_ID)
        .execute(&mut **tx)
        .await?;
        sqlx::query(
            "INSERT INTO chunk_space (chunk_id, space_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
        )
        .bind(&id)
        .bind(&space_id)
        .execute(&mut **tx)
        .await?;
        for tag in &fixture.tags {
            let tag_id: String =
                sqlx::query_scalar("SELECT id FROM tag WHERE user_id = $1 AND name = $2")
                    .bind(DEV_USER_ID)
                    .bind(tag)
                    .fetch_optional(&mut **tx)
                    .await?
                    .ok_or_else(|| {
                        anyhow!("Chunk {} references unknown tag {tag}", fixture.name)
                    })?;
            sqlx::query(
                "INSERT INTO chunk_tag (chunk_id, tag_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
            )
            .bind(&id)
            .bind(tag_id)
            .execute(&mut **tx)
            .await?;
        }
        ids.insert(fixture.name, id);
    }
    Ok(ids)
}
