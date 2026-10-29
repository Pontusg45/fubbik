use anyhow::{Result, bail};

use crate::git::Git;

/// Create and switch to a branch, refusing if the tree is dirty.
pub fn create(git: &Git, name: &str, base: Option<&str>) -> Result<()> {
    if !git.is_clean()? {
        bail!("working tree has uncommitted changes; commit or stash them first");
    }
    let mut args = vec!["switch", "-c", name];
    if let Some(base) = base {
        args.push(base);
    }
    git.run(&args)?;
    Ok(())
}
