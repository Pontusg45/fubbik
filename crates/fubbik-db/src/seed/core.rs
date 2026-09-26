use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::{Postgres, Transaction};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChunkType {
    id: String,
    label: String,
    description: String,
    icon: String,
    color: String,
    display_order: i32,
    examples: serde_json::Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Relation {
    id: String,
    label: String,
    description: String,
    arrow_style: String,
    direction: String,
    color: String,
    display_order: i32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Template {
    id: String,
    name: String,
    description: String,
    #[serde(rename = "type")]
    chunk_type: String,
    content: String,
    priority: i32,
    tags: Vec<String>,
    match_rules: serde_json::Value,
    field_mappings: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
struct Catalog {
    builtin_chunk_types: Vec<ChunkType>,
    builtin_relations: Vec<Relation>,
    builtin_templates: Vec<Template>,
    inverse_pairs: Vec<(String, String)>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize)]
pub struct CoreSeedCounts {
    pub chunk_types: usize,
    pub connection_relations: usize,
    pub templates: usize,
}

/// Upserts the canonical catalog inside the caller's transaction. The JSON is
/// a one-time export of the legacy TypeScript constants, retained as data so
/// production seeding has no JavaScript runtime dependency.
pub async fn seed_core(tx: &mut Transaction<'_, Postgres>) -> Result<CoreSeedCounts> {
    let catalog: Catalog = serde_json::from_str(include_str!("core-catalog.json"))?;
    for item in &catalog.builtin_chunk_types {
        sqlx::query(
            "INSERT INTO chunk_type (id, label, description, icon, color, display_order, examples, built_in) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, true) \
             ON CONFLICT (id) DO UPDATE SET label = EXCLUDED.label, description = EXCLUDED.description, \
             icon = EXCLUDED.icon, color = EXCLUDED.color, display_order = EXCLUDED.display_order, \
             examples = EXCLUDED.examples, built_in = true, updated_at = now()",
        )
        .bind(&item.id)
        .bind(&item.label)
        .bind(&item.description)
        .bind(&item.icon)
        .bind(&item.color)
        .bind(item.display_order)
        .bind(&item.examples)
        .execute(&mut **tx)
        .await?;
    }
    for item in &catalog.builtin_relations {
        sqlx::query(
            "INSERT INTO connection_relation (id, label, description, arrow_style, direction, color, display_order, built_in) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, true) \
             ON CONFLICT (id) DO UPDATE SET label = EXCLUDED.label, description = EXCLUDED.description, \
             arrow_style = EXCLUDED.arrow_style, direction = EXCLUDED.direction, color = EXCLUDED.color, \
             display_order = EXCLUDED.display_order, built_in = true, updated_at = now()",
        )
        .bind(&item.id)
        .bind(&item.label)
        .bind(&item.description)
        .bind(&item.arrow_style)
        .bind(&item.direction)
        .bind(&item.color)
        .bind(item.display_order)
        .execute(&mut **tx)
        .await?;
    }
    for (a, b) in &catalog.inverse_pairs {
        for (from, to) in [(a, b), (b, a)] {
            sqlx::query("UPDATE connection_relation SET inverse_of_id = $2 WHERE id = $1")
                .bind(from)
                .bind(to)
                .execute(&mut **tx)
                .await?;
        }
    }
    for item in &catalog.builtin_templates {
        sqlx::query(
            "INSERT INTO chunk_template (id, name, description, type, content, is_built_in, \
             priority, tags, match_rules, field_mappings) VALUES ($1, $2, $3, $4, $5, true, $6, $7, $8, $9) \
             ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.name, description = EXCLUDED.description, \
             type = EXCLUDED.type, content = EXCLUDED.content, priority = EXCLUDED.priority, \
             tags = EXCLUDED.tags, match_rules = EXCLUDED.match_rules, field_mappings = EXCLUDED.field_mappings",
        )
        .bind(&item.id)
        .bind(&item.name)
        .bind(&item.description)
        .bind(&item.chunk_type)
        .bind(&item.content)
        .bind(item.priority)
        .bind(&item.tags)
        .bind(&item.match_rules)
        .bind(&item.field_mappings)
        .execute(&mut **tx)
        .await?;
    }
    Ok(CoreSeedCounts {
        chunk_types: catalog.builtin_chunk_types.len(),
        connection_relations: catalog.builtin_relations.len(),
        templates: catalog.builtin_templates.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_decodes_all_legacy_builtin_rows() {
        // Given the exported legacy catalog fixture
        let raw = include_str!("core-catalog.json");

        // When the native seed decodes it
        let catalog: Catalog = serde_json::from_str(raw).unwrap();

        // Then it contains the full builtin vocabulary and inverse pairs
        assert_eq!(catalog.builtin_chunk_types.len(), 7);
        assert_eq!(catalog.builtin_relations.len(), 13);
        assert_eq!(catalog.builtin_templates.len(), 5);
        assert_eq!(catalog.inverse_pairs.len(), 5);
    }
}
