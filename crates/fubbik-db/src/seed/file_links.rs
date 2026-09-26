use anyhow::{Result, anyhow};
use serde::Deserialize;
use sqlx::{Postgres, Transaction};
use std::collections::BTreeMap;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AppliesFixture {
    chunk_name: String,
    pattern: String,
    note: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RefFixture {
    chunk_name: String,
    path: String,
    relation: String,
}

#[derive(Deserialize)]
struct FileLinksCatalog {
    applies: Vec<AppliesFixture>,
    refs: Vec<RefFixture>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, serde::Serialize)]
pub struct FileLinkCounts {
    pub applies_to: usize,
    pub file_refs: usize,
}

pub async fn seed_file_links(
    tx: &mut Transaction<'_, Postgres>,
    chunk_ids: &BTreeMap<String, String>,
) -> Result<FileLinkCounts> {
    let catalog: FileLinksCatalog = serde_json::from_str(include_str!("file-links-catalog.json"))?;
    for (index, fixture) in catalog.applies.iter().enumerate() {
        let chunk_id = chunk_ids
            .get(&fixture.chunk_name)
            .ok_or_else(|| anyhow!("Unknown applies-to chunk: {}", fixture.chunk_name))?;
        sqlx::query("DELETE FROM chunk_applies_to WHERE chunk_id = $1 AND pattern = $2")
            .bind(chunk_id)
            .bind(&fixture.pattern)
            .execute(&mut **tx)
            .await?;
        sqlx::query(
            "INSERT INTO chunk_applies_to (id, chunk_id, pattern, note) VALUES ($1, $2, $3, $4)",
        )
        .bind(format!("seed-file-applies-{index}"))
        .bind(chunk_id)
        .bind(&fixture.pattern)
        .bind(&fixture.note)
        .execute(&mut **tx)
        .await?;
    }
    for (index, fixture) in catalog.refs.iter().enumerate() {
        let chunk_id = chunk_ids
            .get(&fixture.chunk_name)
            .ok_or_else(|| anyhow!("Unknown file-ref chunk: {}", fixture.chunk_name))?;
        sqlx::query(
            "DELETE FROM chunk_file_ref WHERE chunk_id = $1 AND path = $2 AND relation = $3",
        )
        .bind(chunk_id)
        .bind(&fixture.path)
        .bind(&fixture.relation)
        .execute(&mut **tx)
        .await?;
        sqlx::query(
            "INSERT INTO chunk_file_ref (id, chunk_id, path, relation) VALUES ($1, $2, $3, $4)",
        )
        .bind(format!("seed-file-ref-{index}"))
        .bind(chunk_id)
        .bind(&fixture.path)
        .bind(&fixture.relation)
        .execute(&mut **tx)
        .await?;
    }
    Ok(FileLinkCounts {
        applies_to: catalog.applies.len(),
        file_refs: catalog.refs.len(),
    })
}
