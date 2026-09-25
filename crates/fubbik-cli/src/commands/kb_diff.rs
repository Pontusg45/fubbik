use anyhow::{Context, Result, bail};
use chrono::{DateTime, Duration, NaiveDate, SecondsFormat, Utc};

use crate::client::{Client, RecapChunk};
use crate::output::{self, OutputMode};

pub async fn run(
    client: &Client,
    since: &str,
    space: Option<&str>,
    mode: OutputMode,
) -> Result<()> {
    let since_date = parse_since(since)?;
    let elapsed = Utc::now().signed_duration_since(since_date);
    let elapsed_seconds = elapsed.num_seconds().max(0);
    let days = ((elapsed_seconds + 86_399) / 86_400) as u32;
    let space_id = client.resolve_space(space).await?;
    let page = client.list_recent_chunks(days, space_id.as_deref()).await?;

    if mode == OutputMode::Json {
        return output::json(&serde_json::json!({
            "since": since_date.to_rfc3339_opts(SecondsFormat::Millis, true),
            "chunks": page.chunks,
            "total": page.total,
        }));
    }
    if mode == OutputMode::Quiet {
        for chunk in page.chunks {
            println!("{}", chunk.id);
        }
        return Ok(());
    }
    if page.chunks.is_empty() {
        println!("No changes since {}", since_date.format("%Y-%m-%d"));
        return Ok(());
    }

    println!(
        "{} chunk(s) changed since {}:\n",
        page.total,
        since_date.format("%Y-%m-%d")
    );
    for chunk in page.chunks {
        let label = if is_new(&chunk) { "NEW" } else { "UPD" };
        let updated = parse_timestamp(&chunk.updated_at)
            .map(|date| date.format("%Y-%m-%d").to_string())
            .unwrap_or_else(|| chunk.updated_at.chars().take(10).collect());
        println!(
            "  {label} {} ({}, {updated})",
            chunk.title, chunk.chunk_type
        );
    }
    Ok(())
}

fn parse_since(value: &str) -> Result<DateTime<Utc>> {
    if value.len() >= 2 {
        let (amount, unit) = value.split_at(value.len() - 1);
        if let Ok(amount) = amount.parse::<i64>() {
            let duration = match unit {
                "d" => Some(Duration::days(amount)),
                "h" => Some(Duration::hours(amount)),
                "w" => Some(Duration::weeks(amount)),
                "m" => Some(Duration::days(amount.saturating_mul(30))),
                _ => None,
            };
            if let Some(duration) = duration {
                return Ok(Utc::now() - duration);
            }
        }
    }
    if let Ok(value) = DateTime::parse_from_rfc3339(value) {
        return Ok(value.with_timezone(&Utc));
    }
    if let Ok(value) = NaiveDate::parse_from_str(value, "%Y-%m-%d") {
        return Ok(value
            .and_hms_opt(0, 0, 0)
            .context("date is outside the supported range")?
            .and_utc());
    }
    bail!("invalid date {value:?}; use an ISO date or a relative period such as 7d")
}

fn parse_timestamp(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|date| date.with_timezone(&Utc))
}

fn is_new(chunk: &RecapChunk) -> bool {
    match (
        parse_timestamp(&chunk.created_at),
        parse_timestamp(&chunk.updated_at),
    ) {
        (Some(created), Some(updated)) => (updated - created).num_seconds().abs() < 60,
        _ => chunk.created_at == chunk.updated_at,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_dates_and_relative_periods_are_accepted() {
        // Given an ISO date, a relative period, and an invalid value
        // When each value is parsed as a knowledge diff boundary
        let iso = parse_since("2026-09-01").unwrap();
        let relative = parse_since("2w").unwrap();
        let invalid = parse_since("last-week");
        // Then valid forms resolve and invalid input produces an actionable error
        assert_eq!(iso.format("%Y-%m-%d").to_string(), "2026-09-01");
        assert!((Utc::now() - relative).num_days() >= 13);
        assert!(invalid.unwrap_err().to_string().contains("invalid date"));
    }

    #[test]
    fn chunks_updated_within_one_minute_of_creation_are_new() {
        // Given chunks updated inside and outside the legacy one-minute threshold
        let mut chunk = RecapChunk {
            id: "chunk-1".into(),
            title: "Prompt".into(),
            chunk_type: "note".into(),
            created_at: "2026-01-01T00:00:00Z".into(),
            updated_at: "2026-01-01T00:00:59Z".into(),
            tags: Vec::new(),
        };
        // When the lifecycle state is classified
        let initially_new = is_new(&chunk);
        chunk.updated_at = "2026-01-01T00:01:00Z".into();
        // Then 59 seconds is new while exactly 60 seconds is updated
        assert!(initially_new);
        assert!(!is_new(&chunk));
    }
}
