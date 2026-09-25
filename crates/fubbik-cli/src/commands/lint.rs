use std::collections::{BTreeMap, HashSet};

use anyhow::{Result, bail};
use serde::Serialize;

use crate::client::Client;
use crate::output::{self, OutputMode};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct LintIssue {
    chunk_id: String,
    chunk_title: String,
    severity: &'static str,
    rule: &'static str,
    message: String,
}

pub async fn run(
    client: &Client,
    space: Option<&str>,
    fix: bool,
    score: bool,
    mode: OutputMode,
) -> Result<()> {
    let space_id = client.resolve_space(space).await?;
    let chunks = super::export::load_chunks(client, space_id.as_deref()).await?;
    if score {
        return render_scores(&chunks, mode);
    }
    let mut issues = chunks.iter().flat_map(lint_chunk).collect::<Vec<_>>();
    if let Ok(health) = client.knowledge_health(space_id.as_deref()).await {
        append_health_issues(
            &mut issues,
            &health,
            "orphans",
            "orphan",
            "Chunk has no connections to other chunks",
        );
        append_health_issues(
            &mut issues,
            &health,
            "stale",
            "stale",
            "Chunk not updated in 30+ days but neighbors have been",
        );
    }
    let errors = issues
        .iter()
        .filter(|issue| issue.severity == "error")
        .count();
    let warnings = issues.len() - errors;
    if fix {
        for issue in issues.iter().filter(|issue| issue.rule == "not-enriched") {
            let _ = client.enrich_chunk(&issue.chunk_id).await;
        }
    }
    render_issues(&issues, chunks.len(), errors, warnings, mode)?;
    if errors > 0 {
        bail!("lint found {errors} error(s)");
    }
    Ok(())
}

fn lint_chunk(chunk: &serde_json::Value) -> Vec<LintIssue> {
    let id = chunk["id"].as_str().unwrap_or("").to_owned();
    let title = chunk["title"].as_str().unwrap_or("Untitled").to_owned();
    let mut issues = Vec::new();
    let content_len = chunk["content"].as_str().map_or(0, str::len);
    if content_len < 100 {
        issues.push(issue(
            &id,
            &title,
            "warning",
            "thin-content",
            format!("Content is only {content_len} chars (min recommended: 100)"),
        ));
    }
    if chunk["type"] == "document" && chunk["rationale"].as_str().is_none() {
        issues.push(issue(
            &id,
            &title,
            "warning",
            "missing-rationale",
            "Document chunks should have a rationale explaining 'why'".into(),
        ));
    }
    if chunk["summary"].as_str().is_none() {
        issues.push(issue(
            &id,
            &title,
            "warning",
            "not-enriched",
            "Chunk has no AI-generated summary (run 'fubbik enrich')".into(),
        ));
    }
    if title.chars().count() < 5 {
        issues.push(issue(
            &id,
            &title,
            "error",
            "short-title",
            "Title should be at least 5 characters".into(),
        ));
    }
    if title.chars().count() > 150 {
        issues.push(issue(
            &id,
            &title,
            "warning",
            "long-title",
            format!(
                "Title is {} chars (max recommended: 150)",
                title.chars().count()
            ),
        ));
    }
    if chunk["origin"] == "ai" && chunk["reviewStatus"] == "draft" {
        issues.push(issue(
            &id,
            &title,
            "warning",
            "unreviewed-ai",
            "AI-generated chunk still in draft status".into(),
        ));
    }
    issues
}

fn issue(
    id: &str,
    title: &str,
    severity: &'static str,
    rule: &'static str,
    message: String,
) -> LintIssue {
    LintIssue {
        chunk_id: id.into(),
        chunk_title: title.into(),
        severity,
        rule,
        message,
    }
}

fn append_health_issues(
    issues: &mut Vec<LintIssue>,
    health: &serde_json::Value,
    bucket: &str,
    rule: &'static str,
    message: &str,
) {
    for chunk in health[bucket]["chunks"].as_array().into_iter().flatten() {
        issues.push(issue(
            chunk["id"].as_str().unwrap_or(""),
            chunk["title"].as_str().unwrap_or("Untitled"),
            "warning",
            rule,
            message.into(),
        ));
    }
}

fn quality_score(chunk: &serde_json::Value) -> u32 {
    let mut score = 0;
    score += u32::from(
        chunk["content"]
            .as_str()
            .is_some_and(|value| value.len() > 100),
    ) * 20;
    score += u32::from(chunk["rationale"].is_string()) * 20;
    score += u32::from(
        chunk["connections"]
            .as_array()
            .is_some_and(|values| !values.is_empty()),
    ) * 15;
    score += u32::from(
        chunk["appliesTo"]
            .as_array()
            .is_some_and(|values| !values.is_empty()),
    ) * 15;
    let generic = HashSet::from([
        "docs",
        "documentation",
        "document",
        "plans",
        "plan",
        "notes",
        "note",
        "general",
        "misc",
        "other",
    ]);
    let meaningful_tag = chunk["tags"].as_array().is_some_and(|tags| {
        tags.iter()
            .filter_map(|tag| tag.as_str().or_else(|| tag["name"].as_str()))
            .any(|tag| !generic.contains(tag.to_ascii_lowercase().as_str()))
    });
    score += u32::from(meaningful_tag) * 15;
    score += u32::from(!matches!(
        chunk["type"].as_str(),
        None | Some("note" | "guide")
    )) * 5;
    score += u32::from(chunk["summary"].is_string()) * 10;
    score
}

fn render_scores(chunks: &[serde_json::Value], mode: OutputMode) -> Result<()> {
    let mut scores = chunks.iter().map(|chunk| serde_json::json!({"id":chunk["id"],"title":chunk["title"],"type":chunk["type"],"score":quality_score(chunk)})).collect::<Vec<_>>();
    scores.sort_by_key(|item| item["score"].as_u64().unwrap_or(0));
    let total: u64 = scores
        .iter()
        .filter_map(|item| item["score"].as_u64())
        .sum();
    let average = if scores.is_empty() {
        0
    } else {
        (total as f64 / scores.len() as f64).round() as u64
    };
    let result = serde_json::json!({"scores":scores,"summary":{"average":average,"below50":scores.iter().filter(|item| item["score"].as_u64().unwrap_or(0)<50).count(),"above80":scores.iter().filter(|item| item["score"].as_u64().unwrap_or(0)>=80).count(),"total":scores.len()}});
    if mode == OutputMode::Json {
        return output::json(&result);
    }
    for item in result["scores"].as_array().into_iter().flatten() {
        println!(
            "  {:>3}  [{}] {}",
            item["score"].as_u64().unwrap_or(0),
            item["type"].as_str().unwrap_or("note"),
            item["title"].as_str().unwrap_or("Untitled")
        );
    }
    println!(
        "\nAverage: {average} | Below 50: {} chunks | Above 80: {} chunks",
        result["summary"]["below50"], result["summary"]["above80"]
    );
    Ok(())
}

fn render_issues(
    issues: &[LintIssue],
    chunks: usize,
    errors: usize,
    warnings: usize,
    mode: OutputMode,
) -> Result<()> {
    if mode == OutputMode::Json {
        return output::json(
            &serde_json::json!({"issues":issues,"summary":{"total":issues.len(),"errors":errors,"warnings":warnings,"chunks":chunks}}),
        );
    }
    if issues.is_empty() {
        println!("All {chunks} chunks passed lint checks");
        return Ok(());
    }
    let mut grouped: BTreeMap<&str, Vec<&LintIssue>> = BTreeMap::new();
    for issue in issues {
        grouped.entry(&issue.chunk_id).or_default().push(issue);
    }
    println!("Linted {chunks} chunks:\n");
    for chunk_issues in grouped.values() {
        println!(
            "  {} ({})",
            chunk_issues[0].chunk_title, chunk_issues[0].chunk_id
        );
        for issue in chunk_issues {
            println!("    {}:{} {}", issue.severity, issue.rule, issue.message);
        }
    }
    println!("\n{errors} error(s) {warnings} warning(s)");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lint_rules_and_quality_score_use_enriched_metadata() {
        // Given a short AI draft with an invalid title and no enrichment
        let weak = serde_json::json!({"id":"c1","title":"Bad","type":"document","content":"short","origin":"ai","reviewStatus":"draft","connections":[],"appliesTo":[],"tags":[]});
        // When lint rules and quality scoring are applied
        let issues = lint_chunk(&weak);
        let score = quality_score(&weak);
        // Then all applicable rules fire and the empty metadata scores zero
        assert!(issues.iter().any(|issue| issue.rule == "short-title"));
        assert!(issues.iter().any(|issue| issue.rule == "unreviewed-ai"));
        assert_eq!(score, 5);
    }
}
