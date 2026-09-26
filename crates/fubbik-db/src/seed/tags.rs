use anyhow::Result;
use serde::Deserialize;
use sqlx::{Postgres, Transaction};

use super::spaces::DEV_USER_ID;

#[derive(Deserialize)]
struct TagCatalog {
    types: Vec<(String, String)>,
    tags: Vec<(String, String)>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, serde::Serialize)]
pub struct TagSeedCounts {
    pub tag_types: usize,
    pub tags: usize,
}

pub async fn seed_tags(tx: &mut Transaction<'_, Postgres>) -> Result<TagSeedCounts> {
    let catalog: TagCatalog = serde_json::from_str(include_str!("tags-catalog.json"))?;
    for (slug, color) in &catalog.types {
        sqlx::query(
            "INSERT INTO tag_type (id, name, color, user_id) VALUES ($1, $2, $3, $4) \
             ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.name, color = EXCLUDED.color",
        )
        .bind(format!("seed-tt-{slug}"))
        .bind(slug)
        .bind(color)
        .bind(DEV_USER_ID)
        .execute(&mut **tx)
        .await?;
    }
    for (name, tag_type) in &catalog.tags {
        sqlx::query(
            "INSERT INTO tag (id, name, tag_type_id, user_id) VALUES ($1, $2, $3, $4) \
             ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.name, tag_type_id = EXCLUDED.tag_type_id",
        )
        .bind(format!("seed-tag-{name}"))
        .bind(name)
        .bind(format!("seed-tt-{tag_type}"))
        .bind(DEV_USER_ID)
        .execute(&mut **tx)
        .await?;
    }
    Ok(TagSeedCounts {
        tag_types: catalog.types.len(),
        tags: catalog.tags.len(),
    })
}
