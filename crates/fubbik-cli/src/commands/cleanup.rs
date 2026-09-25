use std::collections::{HashMap, HashSet};

use anyhow::{Result, bail};
use serde::Serialize;

use crate::client::Client;
use crate::output::{self, OutputMode};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct Candidate {
    id: String,
    title: String,
    #[serde(rename = "type")]
    chunk_type: String,
    reason: String,
    category: &'static str,
}

pub async fn run(
    client: &Client,
    confirm: bool,
    chunk_type: Option<&str>,
    space: Option<&str>,
    mode: OutputMode,
) -> Result<()> {
    let space_id = client.resolve_space(space).await?;
    let chunks = super::export::load_chunks(client, space_id.as_deref()).await?;
    let candidates = find_candidates(&chunks, chunk_type);
    let result = serde_json::json!({
        "candidates": candidates,
        "summary": {
            "planArtifacts": candidates.iter().filter(|item| item.category == "plan-artifact").count(),
            "nearEmpty": candidates.iter().filter(|item| item.category == "near-empty").count(),
            "duplicateTitles": candidates.iter().filter(|item| item.category == "duplicate-title").count(),
            "total": candidates.len(),
        }
    });
    if !confirm {
        return render_analysis(&result, mode);
    }

    let mut removed = Vec::new();
    let mut errors = Vec::new();
    for candidate in &candidates {
        match client.delete_chunk(&candidate.id).await {
            Ok(_) => removed.push(candidate.id.clone()),
            Err(error) => errors.push(format!("{}: {error}", candidate.title)),
        }
    }
    let applied = serde_json::json!({"removed": removed, "errors": errors});
    if mode == OutputMode::Json {
        output::json(&applied)?;
    } else if mode == OutputMode::Quiet {
        for id in &removed {
            println!("{id}");
        }
    } else {
        println!("{} removed, {} failed", removed.len(), errors.len());
    }
    if !errors.is_empty() {
        bail!("{} chunk(s) could not be removed", errors.len());
    }
    Ok(())
}

fn find_candidates(chunks: &[serde_json::Value], chunk_type: Option<&str>) -> Vec<Candidate> {
    let filtered = chunks
        .iter()
        .filter(|chunk| match chunk_type {
            Some(kind) => chunk["type"] == kind,
            None => true,
        })
        .collect::<Vec<_>>();
    let mut candidates = Vec::new();
    let mut seen = HashSet::new();

    for chunk in &filtered {
        let id = chunk["id"].as_str().unwrap_or("");
        let tags = tag_names(chunk);
        if chunk["type"] == "guide" && tags.iter().any(|tag| is_plan_filename(tag)) {
            candidates.push(candidate(
                chunk,
                "Plan task artifact (type: guide, tagged with plan filename)".into(),
                "plan-artifact",
            ));
            seen.insert(id.to_owned());
        } else if chunk["content"].as_str().map_or(0, str::len) < 50 {
            candidates.push(candidate(
                chunk,
                format!(
                    "Near-empty ({} chars)",
                    chunk["content"].as_str().map_or(0, str::len)
                ),
                "near-empty",
            ));
            seen.insert(id.to_owned());
        }
    }

    let mut titles: HashMap<String, Vec<&serde_json::Value>> = HashMap::new();
    for &chunk in &filtered {
        titles
            .entry(
                chunk["title"]
                    .as_str()
                    .unwrap_or("")
                    .trim()
                    .to_ascii_lowercase(),
            )
            .or_default()
            .push(chunk);
    }
    for duplicates in titles.values_mut().filter(|items| items.len() > 1) {
        duplicates.sort_by_key(|chunk| chunk["createdAt"].as_str().unwrap_or(""));
        let copies = duplicates.len();
        for chunk in duplicates.iter().skip(1) {
            let id = chunk["id"].as_str().unwrap_or("");
            if seen.insert(id.to_owned()) {
                candidates.push(candidate(
                    chunk,
                    format!("Duplicate title ({copies} copies)"),
                    "duplicate-title",
                ));
            }
        }
    }
    candidates
}

fn candidate(chunk: &serde_json::Value, reason: String, category: &'static str) -> Candidate {
    Candidate {
        id: chunk["id"].as_str().unwrap_or("").into(),
        title: chunk["title"].as_str().unwrap_or("Untitled").into(),
        chunk_type: chunk["type"].as_str().unwrap_or("note").into(),
        reason,
        category,
    }
}

fn tag_names(chunk: &serde_json::Value) -> Vec<&str> {
    chunk["tags"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|tag| tag.as_str().or_else(|| tag["name"].as_str()))
        .collect()
}

fn is_plan_filename(tag: &str) -> bool {
    let bytes = tag.as_bytes();
    bytes.len() > 5
        && bytes[..4].iter().all(u8::is_ascii_digit)
        && bytes[4] == b'-'
        && tag.ends_with(".md")
}

fn render_analysis(result: &serde_json::Value, mode: OutputMode) -> Result<()> {
    if mode == OutputMode::Json {
        return output::json(result);
    }
    if result["summary"]["total"] == 0 {
        println!("No low-value chunks found");
        return Ok(());
    }
    if mode == OutputMode::Quiet {
        for item in result["candidates"].as_array().into_iter().flatten() {
            println!("{}", item["id"].as_str().unwrap_or(""));
        }
        return Ok(());
    }
    println!("Cleanup analysis:");
    println!(
        "  {} plan task artifacts",
        result["summary"]["planArtifacts"]
    );
    println!("  {} near-empty chunks", result["summary"]["nearEmpty"]);
    println!(
        "  {} duplicate titles",
        result["summary"]["duplicateTitles"]
    );
    println!("\nTotal: {} chunks flagged", result["summary"]["total"]);
    println!("Run 'fubbik cleanup --confirm' to remove.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleanup_prioritizes_artifacts_and_keeps_oldest_duplicate() {
        // Given a plan artifact and two otherwise substantial chunks sharing a title
        let chunks = vec![
            serde_json::json!({"id":"artifact","title":"Task","type":"guide","content":"","createdAt":"2026-01-01","tags":["2026-01-plan.md"]}),
            serde_json::json!({"id":"old","title":"Shared","type":"note","content":"x".repeat(60),"createdAt":"2026-01-01","tags":[]}),
            serde_json::json!({"id":"new","title":" shared ","type":"note","content":"x".repeat(60),"createdAt":"2026-02-01","tags":[]}),
        ];
        // When cleanup candidates are classified
        let candidates = find_candidates(&chunks, None);
        // Then the artifact is not double-counted as empty and only the newer duplicate is removed
        assert_eq!(candidates.len(), 2);
        assert!(
            candidates
                .iter()
                .any(|item| item.id == "artifact" && item.category == "plan-artifact")
        );
        assert!(
            candidates
                .iter()
                .any(|item| item.id == "new" && item.category == "duplicate-title")
        );
    }
}
