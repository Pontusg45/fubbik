use std::process::Command;

use anyhow::{Context, Result, bail};
use serde::Serialize;

use crate::client::Client;
use crate::output::{self, OutputMode};

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct MatchedChunk {
    id: String,
    title: String,
    #[serde(rename = "type")]
    chunk_type: String,
    matched_files: Vec<String>,
    match_reason: String,
}

pub async fn run(
    client: &Client,
    files: Vec<String>,
    staged: bool,
    mode: OutputMode,
) -> Result<()> {
    let files = if staged || files.is_empty() {
        staged_files()?
    } else {
        files
    };
    if files.is_empty() {
        return Ok(());
    }

    let mut matched = Vec::new();
    for file in &files {
        let Ok(value) = client
            .context_for_file(file, None, 8000, "json-legacy")
            .await
        else {
            continue;
        };
        collect_matches(&mut matched, file, &value);
    }
    if matched.is_empty() {
        return Ok(());
    }

    match mode {
        OutputMode::Json => output::json(&matched),
        OutputMode::Quiet => {
            for chunk in matched {
                println!("{}", chunk.id);
            }
            Ok(())
        }
        OutputMode::Human => {
            println!("\n  {} chunk(s) related to your changes:\n", matched.len());
            for chunk in matched {
                println!("  {} [{}]", chunk.title, chunk.chunk_type);
                for file in chunk.matched_files {
                    println!("    {file}");
                }
                println!();
            }
            Ok(())
        }
    }
}

fn staged_files() -> Result<Vec<String>> {
    let output = Command::new("git")
        .args(["diff", "--cached", "--name-only"])
        .output()
        .context("failed to inspect staged files")?;
    if !output.status.success() {
        bail!(
            "git diff --cached failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|file| !file.is_empty())
        .map(str::to_owned)
        .collect())
}

fn collect_matches(matched: &mut Vec<MatchedChunk>, file: &str, value: &serde_json::Value) {
    let chunks = value["chunks"].as_array().into_iter().flatten();
    for chunk in chunks {
        let reason = chunk["matchReason"].as_str().unwrap_or("");
        if !matches!(reason, "file-ref" | "applies-to") {
            continue;
        }
        let id = chunk["id"].as_str().unwrap_or("");
        if id.is_empty() {
            continue;
        }
        if let Some(existing) = matched.iter_mut().find(|existing| existing.id == id) {
            if !existing.matched_files.iter().any(|matched| matched == file) {
                existing.matched_files.push(file.to_owned());
            }
            continue;
        }
        matched.push(MatchedChunk {
            id: id.to_owned(),
            title: chunk["title"].as_str().unwrap_or("Untitled").to_owned(),
            chunk_type: chunk["type"].as_str().unwrap_or("unknown").to_owned(),
            matched_files: vec![file.to_owned()],
            match_reason: reason.to_owned(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_file_matches_are_aggregated_and_indirect_context_is_ignored() {
        // Given direct matches for two files plus semantic context
        let first = serde_json::json!({"chunks": [
            {"id": "c1", "title": "Rule", "type": "convention", "matchReason": "file-ref"},
            {"id": "c2", "title": "Related", "type": "note", "matchReason": "semantic"}
        ]});
        let second = serde_json::json!({"chunks": [
            {"id": "c1", "title": "Rule", "type": "convention", "matchReason": "applies-to"}
        ]});
        // When both responses are collected
        let mut matched = Vec::new();
        collect_matches(&mut matched, "src/a.rs", &first);
        collect_matches(&mut matched, "src/b.rs", &second);
        // Then the direct chunk appears once with both matching paths
        assert_eq!(matched.len(), 1);
        assert_eq!(matched[0].id, "c1");
        assert_eq!(matched[0].matched_files, ["src/a.rs", "src/b.rs"]);
    }
}
