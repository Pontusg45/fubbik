use crate::{
    client::Client,
    output::{self, OutputMode},
};
use anyhow::{Context, Result, bail};
use fubbik_core::source_docs::SourceManifest;
use std::path::Path;

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
        match language {
            "javascript" => crate::source_docs_native::extract_javascript(path, project)?,
            "typescript" => crate::source_docs_native::extract_typescript(path, project)?,
            "java" => crate::source_docs_java::extract(path, project)?,
            _ => bail!("unsupported source language: {language}"),
        }
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
