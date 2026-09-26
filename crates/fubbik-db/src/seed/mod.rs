mod core;
mod planner;

use anyhow::{Result, bail};
use serde::Serialize;
use sqlx::PgPool;

pub use core::{CoreSeedCounts, seed_core};
pub use planner::{MODULES, ResetPolicy, SeedModule, SeedOptions, SeedScenario, plan_modules};

#[derive(Debug, Serialize)]
pub struct SeedReport {
    pub modules: Vec<&'static str>,
    pub core: Option<CoreSeedCounts>,
}

/// Runs selected modules atomically. Reject incomplete ports before making
/// any changes, so maintenance commands cannot silently seed partial data.
pub async fn seed(pool: &PgPool, options: &SeedOptions) -> Result<SeedReport> {
    let modules = plan_modules(MODULES, options).map_err(anyhow::Error::msg)?;
    if let Some(module) = modules.iter().find(|name| **name != "core") {
        bail!("Rust seed module {module} has not been ported yet");
    }

    let mut tx = pool.begin().await?;
    let mut report = SeedReport {
        modules: modules.clone(),
        core: None,
    };
    for module in modules {
        if module == "core" {
            report.core = Some(seed_core(&mut tx).await?);
        }
    }
    tx.commit().await?;
    Ok(report)
}
