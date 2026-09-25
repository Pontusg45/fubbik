use std::collections::{BTreeMap, HashMap, HashSet};

use anyhow::{Result, bail};
use serde::Serialize;

use crate::client::Client;
use crate::output::{self, OutputMode};

const MERGES: &[(&str, &str)] = &[
    ("documentation", "docs"),
    ("document", "docs"),
    ("documents", "docs"),
    ("conventions", "convention"),
    ("architectures", "architecture"),
    ("configurations", "configuration"),
    ("configs", "configuration"),
    ("config", "configuration"),
    ("tests", "testing"),
    ("test", "testing"),
    ("requirements", "requirement"),
    ("implementations", "implementation"),
    ("guides", "guide"),
    ("templates", "template"),
    ("features", "feature"),
    ("migrations", "migration"),
    ("runbooks", "runbook"),
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct TagIssue {
    action: &'static str,
    tag: String,
    merge_to: Option<String>,
    reason: &'static str,
    affected_chunks: usize,
}

pub async fn run(
    client: &Client,
    confirm: bool,
    space: Option<&str>,
    mode: OutputMode,
) -> Result<()> {
    let space_id = client.resolve_space(space).await?;
    let chunks = super::export::load_chunks(client, space_id.as_deref()).await?;
    let issues = analyze(&chunks);
    if !confirm {
        return render(&issues, chunks.len(), mode);
    }

    let issue_map = issues
        .iter()
        .map(|issue| (issue.tag.as_str(), issue))
        .collect::<HashMap<_, _>>();
    let mut applied = 0;
    let mut errors = Vec::new();
    for chunk in &chunks {
        let current = tag_names(chunk);
        let mut next = Vec::new();
        for tag in &current {
            match issue_map.get(tag.as_str()) {
                Some(issue) if issue.action == "merge" => {
                    let target = issue.merge_to.as_ref().expect("merge target").clone();
                    if !next.contains(&target) {
                        next.push(target);
                    }
                }
                Some(_) => {}
                None if !next.contains(tag) => next.push(tag.clone()),
                None => {}
            }
        }
        if next == current {
            continue;
        }
        let id = chunk["id"].as_str().unwrap_or("");
        match client
            .update_chunk(id, None, None, None, Some(&next), None)
            .await
        {
            Ok(_) => applied += 1,
            Err(error) => errors.push(format!(
                "{}: {error}",
                chunk["title"].as_str().unwrap_or(id)
            )),
        }
    }
    let result = serde_json::json!({"applied":applied,"errors":errors,"issues":issues});
    if mode == OutputMode::Json {
        output::json(&result)?;
    } else if mode == OutputMode::Quiet {
        println!("{applied}");
    } else {
        println!(
            "{applied} chunk tag set(s) updated, {} failed",
            errors.len()
        );
    }
    if !errors.is_empty() {
        bail!("{} chunk tag update(s) failed", errors.len());
    }
    Ok(())
}

fn analyze(chunks: &[serde_json::Value]) -> Vec<TagIssue> {
    let mut usage: BTreeMap<String, HashSet<String>> = BTreeMap::new();
    for chunk in chunks {
        let id = chunk["id"].as_str().unwrap_or("").to_owned();
        for tag in tag_names(chunk) {
            usage.entry(tag).or_default().insert(id.clone());
        }
    }
    let mut issues = Vec::new();
    for &(variant, canonical) in MERGES {
        if let Some(ids) = usage.get(variant) {
            issues.push(TagIssue {
                action: "merge",
                tag: variant.into(),
                merge_to: Some(canonical.into()),
                reason: "Variant of canonical tag",
                affected_chunks: ids.len(),
            });
        }
    }
    let broad_threshold = chunks.len() * 7 / 10;
    for (tag, ids) in usage {
        if issues.iter().any(|issue| issue.tag == tag) {
            continue;
        }
        let reason = if ids.len() > broad_threshold {
            Some("Too broad")
        } else if is_filename_tag(&tag) {
            Some("Filename tag")
        } else {
            None
        };
        if let Some(reason) = reason {
            issues.push(TagIssue {
                action: "remove",
                tag,
                merge_to: None,
                reason,
                affected_chunks: ids.len(),
            });
        }
    }
    issues
}

fn tag_names(chunk: &serde_json::Value) -> Vec<String> {
    chunk["tags"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|tag| tag.as_str().or_else(|| tag["name"].as_str()))
        .map(str::to_owned)
        .collect()
}

fn is_filename_tag(tag: &str) -> bool {
    if tag.contains('/') {
        return true;
    }
    let bytes = tag.as_bytes();
    if bytes.len() >= 7
        && bytes[..4].iter().all(u8::is_ascii_digit)
        && bytes[4] == b'-'
        && bytes[5..7].iter().all(u8::is_ascii_digit)
    {
        return true;
    }
    tag.rsplit_once('.').is_some_and(|(_, extension)| {
        (2..=4).contains(&extension.len()) && extension.chars().all(char::is_alphanumeric)
    })
}

fn render(issues: &[TagIssue], total_chunks: usize, mode: OutputMode) -> Result<()> {
    if mode == OutputMode::Json {
        return output::json(&serde_json::json!({"issues":issues,"totalChunks":total_chunks}));
    }
    if issues.is_empty() {
        println!("No tag issues found");
        return Ok(());
    }
    if mode == OutputMode::Quiet {
        for issue in issues {
            println!("{}", issue.tag);
        }
        return Ok(());
    }
    println!("Tag normalization:");
    for issue in issues {
        if let Some(target) = &issue.merge_to {
            println!(
                "  Merge: {} -> {target} ({} chunks affected)",
                issue.tag, issue.affected_chunks
            );
        } else {
            println!(
                "  Remove: {} ({} chunks, {})",
                issue.tag,
                issue.affected_chunks,
                issue.reason.to_ascii_lowercase()
            );
        }
    }
    println!("\nRun 'fubbik tag normalize --confirm' to apply.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalization_detects_variants_broad_tags_and_filenames() {
        // Given three chunks using a variant, an overly broad tag, and a filename tag
        let chunks = vec![
            serde_json::json!({"id":"1","tags":["documentation","general","src/main.rs"]}),
            serde_json::json!({"id":"2","tags":["general"]}),
            serde_json::json!({"id":"3","tags":["general"]}),
        ];
        // When tag issues are analyzed
        let issues = analyze(&chunks);
        // Then each issue receives the intended action
        assert!(
            issues
                .iter()
                .any(|issue| issue.tag == "documentation"
                    && issue.merge_to.as_deref() == Some("docs"))
        );
        assert!(
            issues
                .iter()
                .any(|issue| issue.tag == "general" && issue.reason == "Too broad")
        );
        assert!(
            issues
                .iter()
                .any(|issue| issue.tag == "src/main.rs" && issue.reason == "Filename tag")
        );
    }
}
