use anyhow::{Result, anyhow};
use serde::Deserialize;
use sqlx::{Postgres, Transaction};
use std::collections::BTreeMap;

#[derive(Deserialize)]
struct ConnectionFixture {
    from: String,
    to: String,
    relation: String,
}

pub async fn seed_connections(
    tx: &mut Transaction<'_, Postgres>,
    chunk_ids: &BTreeMap<String, String>,
) -> Result<usize> {
    let fixtures: Vec<ConnectionFixture> =
        serde_json::from_str(include_str!("connections-catalog.json"))?;
    for (index, fixture) in fixtures.iter().enumerate() {
        let source_id = chunk_ids
            .get(&fixture.from)
            .ok_or_else(|| anyhow!("Unknown source chunk: {}", fixture.from))?;
        let target_id = chunk_ids
            .get(&fixture.to)
            .ok_or_else(|| anyhow!("Unknown target chunk: {}", fixture.to))?;
        sqlx::query(
            "INSERT INTO chunk_connection (id, source_id, target_id, relation) \
             VALUES ($1, $2, $3, $4) ON CONFLICT (source_id, target_id, relation) DO NOTHING",
        )
        .bind(format!("seed-connection-{index}"))
        .bind(source_id)
        .bind(target_id)
        .bind(&fixture.relation)
        .execute(&mut **tx)
        .await?;
    }
    Ok(fixtures.len())
}
