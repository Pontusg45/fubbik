mod chunks;
mod connections;
mod coordination;
mod core;
mod file_links;
mod misc;
mod planner;
mod plans;
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
pub use coordination::{CoordinationCounts, seed_coordination};
pub use core::{CoreSeedCounts, seed_core};
pub use file_links::{FileLinkCounts, seed_file_links};
pub use misc::{seed_collections, seed_documents, seed_vocabulary, seed_workspaces};
pub use planner::{MODULES, ResetPolicy, SeedModule, SeedOptions, SeedScenario, plan_modules};
pub use plans::{PlanSeed, seed_plans};
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
    pub plans: Option<usize>,
    pub plan_tasks: Option<usize>,
    pub agent_coordination: Option<CoordinationCounts>,
    pub documents: Option<usize>,
    pub vocabulary: Option<usize>,
    pub workspaces: Option<usize>,
    pub collections: Option<usize>,
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
                | "plans"
                | "agent-coordination"
                | "documents"
                | "vocabulary"
                | "workspaces"
                | "collections"
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
        plans: None,
        plan_tasks: None,
        agent_coordination: None,
        documents: None,
        vocabulary: None,
        workspaces: None,
        collections: None,
        self_documenting: None,
    };
    let mut chunk_ids = std::collections::BTreeMap::new();
    let mut use_case_ids = std::collections::BTreeMap::new();
    let mut requirement_ids = std::collections::BTreeMap::new();
    let mut plan_seed = None;
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
            requirement_ids = seed_requirements(&mut tx, &use_case_ids, &chunk_ids).await?;
            report.requirements = Some(requirement_ids.len());
        } else if module == "plans" {
            let plan = seed_plans(&mut tx, &requirement_ids, &chunk_ids).await?;
            report.plans = Some(1);
            report.plan_tasks = Some(plan.task_ids.len());
            plan_seed = Some(plan);
        } else if module == "agent-coordination" {
            let plan = plan_seed
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Missing seed plan"))?;
            report.agent_coordination = Some(seed_coordination(&mut tx, plan).await?);
        } else if module == "documents" {
            report.documents = Some(seed_documents(&mut tx).await?);
        } else if module == "vocabulary" {
            report.vocabulary = Some(seed_vocabulary(&mut tx).await?);
        } else if module == "workspaces" {
            report.workspaces = Some(seed_workspaces(&mut tx).await?);
        } else if module == "collections" {
            report.collections = Some(seed_collections(&mut tx).await?);
        } else if module == "self-documenting" {
            report.self_documenting = Some(seed_self_documenting(&mut tx).await?);
        }
    }
    tx.commit().await?;
    Ok(report)
}
