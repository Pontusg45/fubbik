use std::io::Read;
use std::path::Path;

use anyhow::{Context, Result};

pub fn read(
    inline: Option<String>,
    file: Option<&Path>,
    stdin: bool,
    empty_when_missing: bool,
) -> Result<Option<String>> {
    if let Some(value) = inline {
        return Ok(Some(value));
    }
    if let Some(path) = file {
        return std::fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))
            .map(Some);
    }
    if stdin {
        let mut value = String::new();
        std::io::stdin()
            .read_to_string(&mut value)
            .context("failed to read stdin")?;
        return Ok(Some(value));
    }
    Ok(empty_when_missing.then(String::new))
}
