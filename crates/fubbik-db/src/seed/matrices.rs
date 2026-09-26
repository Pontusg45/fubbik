use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use sqlx::{Postgres, Transaction};
use std::collections::BTreeMap;

use super::spaces::DEV_USER_ID;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Catalog {
    matrices: Vec<MatrixFixture>,
    requirement_links: Vec<RequirementLink>,
}

#[derive(Deserialize)]
struct MatrixFixture {
    name: String,
    layer: String,
    description: String,
    dimensions: Vec<DimensionFixture>,
    rules: Vec<RuleFixture>,
    cells: Vec<CellFixture>,
}

#[derive(Deserialize)]
struct DimensionFixture {
    key: String,
    name: String,
}

#[derive(Deserialize)]
struct RuleFixture {
    key: String,
    title: String,
    category: String,
    description: Option<String>,
}

#[derive(Deserialize)]
struct CellFixture {
    key: String,
    rule: String,
    dim: String,
}

#[derive(Deserialize)]
struct RequirementLink {
    cell: String,
    req: String,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize)]
pub struct MatrixCounts {
    pub matrices: usize,
    pub dimensions: usize,
    pub rules: usize,
    pub cells: usize,
    pub requirement_links: usize,
}

pub async fn seed_matrices(
    tx: &mut Transaction<'_, Postgres>,
    requirement_ids: &BTreeMap<String, String>,
) -> Result<MatrixCounts> {
    let catalog: Catalog = serde_json::from_str(include_str!("matrices-catalog.json"))?;
    let space_id: String =
        sqlx::query_scalar("SELECT id FROM space WHERE user_id = $1 AND name = 'fubbik'")
            .bind(DEV_USER_ID)
            .fetch_one(&mut **tx)
            .await?;
    let mut dimensions = BTreeMap::new();
    let mut rules = BTreeMap::new();
    let mut cells = BTreeMap::new();
    let mut counts = MatrixCounts {
        matrices: catalog.matrices.len(),
        dimensions: 0,
        rules: 0,
        cells: 0,
        requirement_links: 0,
    };
    for matrix in catalog.matrices {
        let slug = if matrix.layer == "invariant" {
            "invariants"
        } else {
            "contracts"
        };
        let existing: Option<String> =
            sqlx::query_scalar("SELECT id FROM behavior_matrix WHERE user_id = $1 AND name = $2")
                .bind(DEV_USER_ID)
                .bind(&matrix.name)
                .fetch_optional(&mut **tx)
                .await?;
        let matrix_id = existing.unwrap_or_else(|| format!("seed-matrix-{slug}"));
        sqlx::query(
            "INSERT INTO behavior_matrix (id, name, layer, description, space_id, user_id) \
             VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT (id) DO UPDATE SET \
             description = EXCLUDED.description, updated_at = now()",
        )
        .bind(&matrix_id)
        .bind(&matrix.name)
        .bind(&matrix.layer)
        .bind(&matrix.description)
        .bind(&space_id)
        .bind(DEV_USER_ID)
        .execute(&mut **tx)
        .await?;
        for (order, dimension) in matrix.dimensions.iter().enumerate() {
            let existing: Option<String> = sqlx::query_scalar(
                "SELECT id FROM behavior_dimension WHERE matrix_id = $1 AND name = $2",
            )
            .bind(&matrix_id)
            .bind(&dimension.name)
            .fetch_optional(&mut **tx)
            .await?;
            let id = existing.unwrap_or_else(|| format!("seed-matrix-dimension-{}", dimension.key));
            sqlx::query(
                "INSERT INTO behavior_dimension (id, matrix_id, name, \"order\") \
                 VALUES ($1, $2, $3, $4) ON CONFLICT (id) DO UPDATE SET \"order\" = EXCLUDED.\"order\"",
            )
            .bind(&id).bind(&matrix_id).bind(&dimension.name).bind(order as i32)
            .execute(&mut **tx).await?;
            if dimensions.insert(dimension.key.clone(), id).is_some() {
                return Err(anyhow!("Duplicate matrix dimension key: {}", dimension.key));
            }
            counts.dimensions += 1;
        }
        for (order, rule) in matrix.rules.iter().enumerate() {
            let existing: Option<String> = sqlx::query_scalar(
                "SELECT id FROM behavior_rule WHERE matrix_id = $1 AND title = $2",
            )
            .bind(&matrix_id)
            .bind(&rule.title)
            .fetch_optional(&mut **tx)
            .await?;
            let id = existing.unwrap_or_else(|| format!("seed-matrix-rule-{}", rule.key));
            sqlx::query(
                "INSERT INTO behavior_rule (id, matrix_id, title, description, category, \"order\") \
                 VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT (id) DO UPDATE SET \
                 description = EXCLUDED.description, category = EXCLUDED.category, \"order\" = EXCLUDED.\"order\"",
            )
            .bind(&id).bind(&matrix_id).bind(&rule.title).bind(&rule.description)
            .bind(&rule.category).bind(order as i32).execute(&mut **tx).await?;
            if rules.insert(rule.key.clone(), id).is_some() {
                return Err(anyhow!("Duplicate matrix rule key: {}", rule.key));
            }
            counts.rules += 1;
        }
        for cell in &matrix.cells {
            let rule_id = rules
                .get(&cell.rule)
                .ok_or_else(|| anyhow!("Unknown matrix rule: {}", cell.rule))?;
            let dimension_id = dimensions
                .get(&cell.dim)
                .ok_or_else(|| anyhow!("Unknown matrix dimension: {}", cell.dim))?;
            let existing: Option<String> = sqlx::query_scalar(
                "SELECT id FROM behavior_cell WHERE rule_id = $1 AND dimension_id = $2",
            )
            .bind(rule_id)
            .bind(dimension_id)
            .fetch_optional(&mut **tx)
            .await?;
            let id = existing.unwrap_or_else(|| format!("seed-matrix-cell-{}", cell.key));
            sqlx::query(
                "INSERT INTO behavior_cell (id, rule_id, dimension_id) VALUES ($1, $2, $3) \
                 ON CONFLICT (id) DO NOTHING",
            )
            .bind(&id)
            .bind(rule_id)
            .bind(dimension_id)
            .execute(&mut **tx)
            .await?;
            if cells.insert(cell.key.clone(), id).is_some() {
                return Err(anyhow!("Duplicate matrix cell key: {}", cell.key));
            }
            counts.cells += 1;
        }
    }
    for link in &catalog.requirement_links {
        let cell_id = cells
            .get(&link.cell)
            .ok_or_else(|| anyhow!("Unknown matrix cell: {}", link.cell))?;
        let requirement_id = requirement_ids
            .get(&link.req)
            .ok_or_else(|| anyhow!("Unknown requirement: {}", link.req))?;
        sqlx::query(
            "INSERT INTO behavior_cell_requirement (cell_id, requirement_id) \
             VALUES ($1, $2) ON CONFLICT DO NOTHING",
        )
        .bind(cell_id)
        .bind(requirement_id)
        .execute(&mut **tx)
        .await?;
        counts.requirement_links += 1;
    }
    Ok(counts)
}
