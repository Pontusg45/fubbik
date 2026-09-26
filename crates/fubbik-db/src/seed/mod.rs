mod chunks;
mod connections;
mod core;
mod file_links;
mod planner;
mod requirements;
mod self_documenting;
mod spaces;
mod tags;
mod use_cases;

use anyhow::{Result, bail};
use serde::Serialize;
use sqlx::PgPool;

pub use chunks::seed_chunks;
pub use connections::seed_connections;
pub use core::{CoreSeedCounts, seed_core};
pub use file_links::{FileLinkCounts, seed_file_links};
pub use planner::{MODULES, ResetPolicy, SeedModule, SeedOptions, SeedScenario, plan_modules};
pub use requirements::seed_requirements;
pub use self_documenting::{SelfDocCounts, seed_self_documenting};
pub use spaces::seed_spaces;
pub use tags::{TagSeedCounts, seed_tags};
pub use use_cases::seed_use_cases;

#[derive(Debug, Serialize)]
pub struct SeedReport {
    pub modules: Vec<&'static str>,
    pub core: Option<CoreSeedCounts>,
    pub spaces: Option<usize>,
    pub tags: Option<TagSeedCounts>,
    pub chunks: Option<usize>,
    pub connections: Option<usize>,
    pub file_links: Option<FileLinkCounts>,
    pub use_cases: Option<usize>,
    pub requirements: Option<usize>,
    pub self_documenting: Option<SelfDocCounts>,
}

/// Runs selected modules atomically. Reject incomplete ports before making
/// any changes, so maintenance commands cannot silently seed partial data.
pub async fn seed(pool: &PgPool, options: &SeedOptions) -> Result<SeedReport> {
    let modules = plan_modules(MODULES, options).map_err(anyhow::Error::msg)?;
    if let Some(module) = modules.iter().find(|name| {
        !matches!(
            **name,
            "core"
                | "codebases"
                | "tags"
                | "chunks"
                | "connections"
                | "file-links"
                | "use-cases"
                | "requirements"
                | "self-documenting"
        )
    }) {
        bail!("Rust seed module {module} has not been ported yet");
    }

    let mut tx = pool.begin().await?;
    sqlx::query(
        "INSERT INTO \"user\" (id, name, email, email_verified) \
         VALUES ($1, 'Dev User', 'dev@localhost', false) ON CONFLICT (id) DO NOTHING",
    )
    .bind(spaces::DEV_USER_ID)
    .execute(&mut *tx)
    .await?;
    let mut report = SeedReport {
        modules: modules.clone(),
        core: None,
        spaces: None,
        tags: None,
        chunks: None,
        connections: None,
        file_links: None,
        use_cases: None,
        requirements: None,
        self_documenting: None,
    };
    let mut chunk_ids = std::collections::BTreeMap::new();
    let mut use_case_ids = std::collections::BTreeMap::new();
    for module in modules {
        if module == "core" {
            report.core = Some(seed_core(&mut tx).await?);
        } else if module == "codebases" {
            report.spaces = Some(seed_spaces(&mut tx).await?);
        } else if module == "tags" {
            report.tags = Some(seed_tags(&mut tx).await?);
        } else if module == "chunks" {
            chunk_ids = seed_chunks(&mut tx).await?;
            report.chunks = Some(chunk_ids.len());
        } else if module == "connections" {
            report.connections = Some(seed_connections(&mut tx, &chunk_ids).await?);
        } else if module == "file-links" {
            report.file_links = Some(seed_file_links(&mut tx, &chunk_ids).await?);
        } else if module == "use-cases" {
            use_case_ids = seed_use_cases(&mut tx).await?;
            report.use_cases = Some(use_case_ids.len());
        } else if module == "requirements" {
            report.requirements = Some(
                seed_requirements(&mut tx, &use_case_ids, &chunk_ids)
                    .await?
                    .len(),
            );
        } else if module == "self-documenting" {
            report.self_documenting = Some(seed_self_documenting(&mut tx).await?);
        }
    }
    tx.commit().await?;
    Ok(report)
}
