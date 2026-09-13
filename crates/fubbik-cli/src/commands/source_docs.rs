use crate::{
    client::Client,
    output::{self, OutputMode},
};
use anyhow::{Context, Result, bail};
use fubbik_core::source_docs::SourceManifest;
use std::{path::Path, process::Command};

pub async fn run(
    client: &Client,
    path: &Path,
    language: Option<&str>,
    project: Option<&str>,
    space: Option<&str>,
    preview: bool,
    mode: OutputMode,
) -> Result<()> {
    let manifest: SourceManifest = if let Some(language) = language {
        let project = project.context("--project is required when extracting source")?;
        let temporary = tempfile::tempdir()?;
        let script = temporary.path().join("extract.mjs");
        std::fs::write(&script, include_str!("../source-docs/extract.mjs"))?;
        std::fs::write(
            temporary.path().join("FubbikDoclet.java"),
            include_str!("../source-docs/FubbikDoclet.java"),
        )?;
        let result = Command::new("node")
            .arg(&script)
            .arg(path.canonicalize()?)
            .arg(language)
            .arg(project)
            .arg(temporary.path())
            .output()
            .context("source extraction requires Node.js on PATH")?;
        if !result.status.success() {
            bail!(
                "source extraction failed: {}",
                String::from_utf8_lossy(&result.stderr)
            );
        }
        if !result.stderr.is_empty() {
            eprint!("{}", String::from_utf8_lossy(&result.stderr));
        }
        serde_json::from_slice(&result.stdout).context("extractor returned an invalid manifest")?
    } else {
        let metadata = std::fs::metadata(path)?;
        if metadata.len() > 16 * 1024 * 1024 {
            bail!("manifest exceeds 16 MiB");
        }
        serde_json::from_slice(&std::fs::read(path)?).context("invalid source manifest JSON")?
    };
    manifest.validate().map_err(anyhow::Error::msg)?;
    if preview {
        return output::json(&manifest);
    }
    let space_id = client
        .resolve_space(space)
        .await?
        .context("source documentation requires a space (--space)")?;
    let result = client.import_source_docs(&space_id, &manifest).await?;
    let id = result["documentId"].as_str().unwrap_or("");
    if !output::id_or_json(mode, id, &result)? {
        println!(
            "Imported source documentation {id}: {} created, {} updated, {} unchanged, {} missing",
            result["created"], result["updated"], result["unchanged"], result["missing"]
        );
    }
    if let Some(conflicts) = result["conflicts"]
        .as_array()
        .filter(|items| !items.is_empty())
    {
        bail!(
            "{} source documentation conflicts; human edits were preserved: {}",
            conflicts.len(),
            conflicts
                .iter()
                .filter_map(|item| item.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    Ok(())
}
