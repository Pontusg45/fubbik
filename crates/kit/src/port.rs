//! `fk port kill`: free a TCP port by killing whatever is using it.

use std::io::ErrorKind;
use std::process::Command;

use anyhow::{Context, Result, bail};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum KillOutcome {
    Free,
    Killed { pids: Vec<u32> },
    WouldKill { pids: Vec<u32> },
}

/// Pids listening on `port`, from `lsof -ti tcp:<port> -sTCP:LISTEN`.
/// Clients connected to the port are not matched. Empty when nothing listens.
pub fn pids(port: u16) -> Result<Vec<u32>> {
    let out = Command::new("lsof")
        .args(["-ti", &format!("tcp:{port}"), "-sTCP:LISTEN"])
        .output()
        .map_err(|e| {
            if e.kind() == ErrorKind::NotFound {
                anyhow::anyhow!("lsof not found; fk port needs lsof")
            } else {
                anyhow::Error::new(e).context("failed to run lsof")
            }
        })?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    // lsof exits 1 with empty output when nothing matches.
    if !out.status.success() && (out.status.code() != Some(1) || !stdout.trim().is_empty()) {
        bail!(
            "lsof failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    let mut pids: Vec<u32> = stdout
        .lines()
        .filter_map(|l| l.trim().parse().ok())
        .collect();
    pids.sort_unstable();
    pids.dedup();
    Ok(pids)
}

/// SIGKILL everything on `port` (or just report it with `dry_run`).
pub fn kill(port: u16, dry_run: bool) -> Result<KillOutcome> {
    let pids = pids(port)?;
    if pids.is_empty() {
        return Ok(KillOutcome::Free);
    }
    if dry_run {
        return Ok(KillOutcome::WouldKill { pids });
    }
    for pid in &pids {
        let status = Command::new("kill")
            .args(["-9", &pid.to_string()])
            .status()
            .context("failed to run kill")?;
        if !status.success() {
            bail!("kill -9 {pid} failed on port {port}");
        }
    }
    Ok(KillOutcome::Killed { pids })
}

pub fn describe(port: u16, outcome: &KillOutcome) -> String {
    let join = |pids: &[u32]| {
        pids.iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    };
    match outcome {
        KillOutcome::Free => format!("port {port} is free"),
        KillOutcome::Killed { pids } => format!("killed {} on port {port}", join(pids)),
        KillOutcome::WouldKill { pids } => format!("would kill {} on port {port}", join(pids)),
    }
}
