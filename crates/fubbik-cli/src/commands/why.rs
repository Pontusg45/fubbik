use std::path::Path;

use anyhow::Result;

use crate::client::Client;
use crate::output::{self, OutputMode};

pub async fn run(
    client: &Client,
    path: &Path,
    space: Option<&str>,
    mode: OutputMode,
) -> Result<()> {
    let space = client.resolve_space(space).await?;
    let value = client
        .context_for_file(
            &path.to_string_lossy(),
            space.as_deref(),
            8000,
            "structured-json",
        )
        .await?;
    let chunks = value["sections"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|section| section["chunks"].as_array().into_iter().flatten())
        .cloned()
        .collect::<Vec<_>>();
    let (reasoning, context): (Vec<_>, Vec<_>) = chunks.into_iter().partition(is_reasoning);
    let result = serde_json::json!({
        "path": path,
        "reasoning": reasoning,
        "context": context,
    });
    if mode == OutputMode::Json {
        return output::json(&result);
    }
    println!("Why: {}\n", path.display());
    if reasoning.is_empty() && context.is_empty() {
        println!("No knowledge found for this file.");
        return Ok(());
    }
    print_group("Decisions & Conventions", reasoning);
    print_group("Related context", context);
    Ok(())
}

fn print_group(label: &str, chunks: Vec<serde_json::Value>) {
    if chunks.is_empty() {
        return;
    }
    println!("{label} ({}):", chunks.len());
    for chunk in chunks {
        println!(
            "  [{}] {}",
            chunk["type"].as_str().unwrap_or("unknown"),
            chunk["title"].as_str().unwrap_or("Untitled")
        );
    }
    println!();
}

fn is_reasoning(chunk: &serde_json::Value) -> bool {
    if chunk["rationale"].is_string()
        || chunk["consequences"].is_string()
        || chunk["alternatives"]
            .as_array()
            .is_some_and(|v| !v.is_empty())
        || chunk["type"] == "convention"
    {
        return true;
    }
    chunk["tags"].as_array().is_some_and(|tags| {
        tags.iter().any(|tag| {
            let name = tag.as_str().or_else(|| tag["name"].as_str()).unwrap_or("");
            ["architecture", "decision", "convention", "rationale", "why"]
                .contains(&name.to_ascii_lowercase().as_str())
        })
    })
}

#[cfg(test)]
mod tests {
    use super::is_reasoning;

    #[test]
    fn reasoning_classification_recognizes_metadata_and_decision_tags() {
        // Given chunks with rationale, a decision tag, and ordinary content
        let rationale = serde_json::json!({"rationale": "Because"});
        let decision = serde_json::json!({"tags": [{"name": "Decision"}]});
        let ordinary = serde_json::json!({"type": "note", "tags": []});

        // When each chunk is classified
        let classifications = [
            is_reasoning(&rationale),
            is_reasoning(&decision),
            is_reasoning(&ordinary),
        ];

        // Then only chunks carrying decision evidence count as reasoning
        assert_eq!(classifications, [true, true, false]);
    }
}
