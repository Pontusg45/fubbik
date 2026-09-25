use std::path::Path;

use anyhow::{Context, Result, bail};

use crate::client::Client;
use crate::output::{self, OutputMode};

pub async fn run(
    client: &Client,
    format: &str,
    out: &Path,
    space: Option<&str>,
    mode: OutputMode,
) -> Result<()> {
    let space_id = client.resolve_space(space).await?;
    let chunks = load_chunks(client, space_id.as_deref()).await?;
    match format {
        "json" => output::json(&chunks),
        "md" => export_markdown(&chunks, out, mode),
        _ => bail!("unknown format {format:?}; use json or md"),
    }
}

pub async fn load_chunks(
    client: &Client,
    space_id: Option<&str>,
) -> Result<Vec<serde_json::Value>> {
    let mut offset = 0_u32;
    let mut summaries = Vec::new();
    loop {
        let page = client.list_chunk_page(space_id, offset).await?;
        let count = page.chunks.len();
        summaries.extend(page.chunks);
        offset = offset.saturating_add(count as u32);
        if count == 0 || i64::from(offset) >= page.total {
            break;
        }
    }

    let mut chunks = Vec::with_capacity(summaries.len());
    for summary in summaries {
        chunks.push(flatten_detail(client.get_chunk_detail(&summary.id).await?));
    }
    Ok(chunks)
}

fn flatten_detail(mut detail: serde_json::Value) -> serde_json::Value {
    let chunk = detail.get_mut("chunk").map(serde_json::Value::take);
    let Some(serde_json::Value::Object(mut chunk)) = chunk else {
        return detail;
    };
    if let serde_json::Value::Object(metadata) = detail {
        for (key, value) in metadata {
            if key != "chunk" {
                chunk.insert(key, value);
            }
        }
    }
    serde_json::Value::Object(chunk)
}

fn export_markdown(chunks: &[serde_json::Value], out: &Path, mode: OutputMode) -> Result<()> {
    std::fs::create_dir_all(out).with_context(|| format!("could not create {}", out.display()))?;
    for chunk in chunks {
        let id = chunk["id"].as_str().context("export chunk omitted id")?;
        let path = out.join(format!("{id}.md"));
        std::fs::write(&path, markdown(chunk))
            .with_context(|| format!("could not write {}", path.display()))?;
    }
    match mode {
        OutputMode::Json => output::json(&serde_json::json!({"count": chunks.len(), "dir": out})),
        OutputMode::Quiet => {
            println!("{}", out.display());
            Ok(())
        }
        OutputMode::Human => {
            println!("Exported {} chunk(s) to {}/", chunks.len(), out.display());
            Ok(())
        }
    }
}

fn markdown(chunk: &serde_json::Value) -> String {
    let tags = chunk["tags"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|tag| tag.as_str().or_else(|| tag["name"].as_str()))
        .map(|tag| format!("\"{}\"", tag.replace('"', "\\\"")))
        .collect::<Vec<_>>()
        .join(", ");
    let title = chunk["title"].as_str().unwrap_or("Untitled");
    format!(
        "---\nid: {}\ntitle: \"{}\"\ntype: {}\ntags: [{tags}]\ncreatedAt: {}\nupdatedAt: {}\n---\n\n# {title}\n\n{}\n",
        chunk["id"].as_str().unwrap_or(""),
        title.replace('"', "\\\""),
        chunk["type"].as_str().unwrap_or("note"),
        chunk["createdAt"].as_str().unwrap_or(""),
        chunk["updatedAt"].as_str().unwrap_or(""),
        chunk["content"].as_str().unwrap_or("")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_export_accepts_string_and_object_tags() {
        // Given an enriched chunk with both supported tag wire shapes
        let chunk = serde_json::json!({
            "id": "c1", "title": "A \"quoted\" title", "type": "note",
            "content": "Body", "createdAt": "one", "updatedAt": "two",
            "tags": ["plain", {"name": "object"}]
        });
        // When the chunk is rendered as Markdown
        let rendered = markdown(&chunk);
        // Then frontmatter is escaped and both tag names are retained
        assert!(rendered.contains("title: \"A \\\"quoted\\\" title\""));
        assert!(rendered.contains("tags: [\"plain\", \"object\"]"));
        assert!(rendered.ends_with("Body\n"));
    }
}
