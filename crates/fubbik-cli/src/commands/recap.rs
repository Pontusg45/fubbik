use std::collections::BTreeMap;

use anyhow::Result;
use chrono::{Duration, SecondsFormat, Utc};
use serde::Serialize;

use crate::client::{Client, RecapChunk};
use crate::output::{self, OutputMode};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Recap {
    since: String,
    new_chunks: usize,
    updated_chunks: usize,
    by_type: BTreeMap<String, Vec<RecapChunk>>,
    chunks: Vec<RecapChunk>,
}

pub async fn run(
    client: &Client,
    since: &str,
    space: Option<&str>,
    mode: OutputMode,
) -> Result<()> {
    let days = parse_to_days(since);
    let since =
        (Utc::now() - Duration::days(i64::from(days))).to_rfc3339_opts(SecondsFormat::Millis, true);
    let space_id = client.resolve_space(space).await?;
    let page = client.list_recent_chunks(days, space_id.as_deref()).await?;
    let recap = summarize(page.chunks, since);

    match mode {
        OutputMode::Json => output::json(&recap),
        OutputMode::Quiet => {
            for chunk in &recap.chunks {
                println!("{}", chunk.id);
            }
            Ok(())
        }
        OutputMode::Human => {
            render(&recap, days);
            Ok(())
        }
    }
}

fn parse_to_days(value: &str) -> u32 {
    let Some((amount, unit)) = value.split_at_checked(value.len().saturating_sub(1)) else {
        return 7;
    };
    let Ok(amount) = amount.parse::<u32>() else {
        return 7;
    };
    match unit {
        "d" => amount,
        "h" => amount.div_ceil(24),
        "w" => amount.saturating_mul(7),
        "m" => amount.saturating_mul(30),
        _ => 7,
    }
}

fn summarize(chunks: Vec<RecapChunk>, since: String) -> Recap {
    let new_chunks = chunks
        .iter()
        .filter(|chunk| chunk.created_at == chunk.updated_at)
        .count();
    let mut by_type = BTreeMap::new();
    for chunk in &chunks {
        by_type
            .entry(chunk.chunk_type.clone())
            .or_insert_with(Vec::new)
            .push(chunk.clone());
    }
    Recap {
        since,
        new_chunks,
        updated_chunks: chunks.len() - new_chunks,
        by_type,
        chunks,
    }
}

fn render(recap: &Recap, days: u32) {
    let suffix = if days == 1 { "" } else { "s" };
    println!("Knowledge base recap (last {days} day{suffix}, from server):");
    println!(
        "  {} new chunk{}, {} updated",
        recap.new_chunks,
        if recap.new_chunks == 1 { "" } else { "s" },
        recap.updated_chunks
    );

    let new_chunks: Vec<_> = recap
        .chunks
        .iter()
        .filter(|chunk| chunk.created_at == chunk.updated_at)
        .collect();
    if !new_chunks.is_empty() {
        println!("\nNew:");
        for chunk in new_chunks.iter().take(20) {
            println!("  + [{}] {}", chunk.chunk_type, chunk.title);
        }
        if new_chunks.len() > 20 {
            println!("  ... and {} more", new_chunks.len() - 20);
        }
    }

    let updated: Vec<_> = recap
        .chunks
        .iter()
        .filter(|chunk| chunk.created_at != chunk.updated_at)
        .collect();
    if !updated.is_empty() {
        println!("\nUpdated:");
        for chunk in updated.iter().take(10) {
            println!("  ~ [{}] {}", chunk.chunk_type, chunk.title);
        }
        if updated.len() > 10 {
            println!("  ... and {} more", updated.len() - 10);
        }
    }

    if !recap.by_type.is_empty() {
        println!("\nBy type:");
        for (chunk_type, chunks) in &recap.by_type {
            println!("  {chunk_type}: {}", chunks.len());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(id: &str, chunk_type: &str, created_at: &str, updated_at: &str) -> RecapChunk {
        RecapChunk {
            id: id.into(),
            title: id.into(),
            chunk_type: chunk_type.into(),
            created_at: created_at.into(),
            updated_at: updated_at.into(),
            tags: Vec::new(),
        }
    }

    #[test]
    fn relative_periods_are_converted_to_api_days() {
        // Given relative periods supported by the legacy command
        // When they are converted to the API's day-based filter
        // Then partial days round up and invalid values retain the seven-day default
        assert_eq!(parse_to_days("25h"), 2);
        assert_eq!(parse_to_days("2w"), 14);
        assert_eq!(parse_to_days("1m"), 30);
        assert_eq!(parse_to_days("2026-01-01"), 7);
    }

    #[test]
    fn recap_separates_new_and_updated_chunks_and_groups_types() {
        // Given one newly created chunk and one later update
        let chunks = vec![
            chunk("new", "note", "same", "same"),
            chunk("changed", "decision", "before", "after"),
        ];
        // When the recap is summarized
        let recap = summarize(chunks, "2026-01-01T00:00:00Z".into());
        // Then lifecycle counts and type groups describe both chunks
        assert_eq!(recap.new_chunks, 1);
        assert_eq!(recap.updated_chunks, 1);
        assert_eq!(recap.by_type["decision"].len(), 1);
        assert_eq!(recap.by_type["note"].len(), 1);
    }
}
