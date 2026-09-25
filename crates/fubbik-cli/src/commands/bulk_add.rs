use std::io::Read;
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::client::Client;
use crate::output::{self, OutputMode};

#[derive(Debug, Deserialize)]
struct ImportChunk {
    title: Option<String>,
    #[serde(default)]
    content: String,
    #[serde(rename = "type", default = "default_type")]
    chunk_type: String,
    #[serde(default)]
    tags: Vec<String>,
}

#[derive(Serialize)]
struct AddedChunk {
    id: String,
    title: String,
}

#[derive(Serialize)]
struct ImportError {
    line: usize,
    error: String,
}

fn default_type() -> String {
    "note".into()
}

pub async fn run(client: &Client, file: &Path, mode: OutputMode) -> Result<()> {
    let raw = read_input(file)?;
    let mut added = Vec::new();
    let mut errors = Vec::new();

    for (index, line) in raw.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let parsed = match parse_line(line) {
            Ok(chunk) => chunk,
            Err(error) => {
                errors.push(ImportError {
                    line: index + 1,
                    error: error.to_string(),
                });
                continue;
            }
        };
        match client
            .create_chunk(
                parsed.title.as_deref().expect("validated title"),
                &parsed.content,
                &parsed.chunk_type,
                &parsed.tags,
                &[],
            )
            .await
        {
            Ok(chunk) => added.push(AddedChunk {
                id: chunk.id,
                title: chunk.title,
            }),
            Err(error) => errors.push(ImportError {
                line: index + 1,
                error: error.to_string(),
            }),
        }
    }

    match mode {
        OutputMode::Json => output::json(&serde_json::json!({"added": added, "errors": errors}))?,
        OutputMode::Quiet => {
            for chunk in &added {
                println!("{}", chunk.id);
            }
        }
        OutputMode::Human => {
            println!("Added {} chunk(s)", added.len());
            if !errors.is_empty() {
                println!("{} error(s):", errors.len());
                for error in &errors {
                    println!("  Line {}: {}", error.line, error.error);
                }
            }
        }
    }
    if !errors.is_empty() {
        bail!("{} JSONL line(s) could not be imported", errors.len());
    }
    Ok(())
}

fn read_input(file: &Path) -> Result<String> {
    if file == Path::new("-") {
        let mut raw = String::new();
        std::io::stdin()
            .read_to_string(&mut raw)
            .context("failed to read JSONL from stdin")?;
        return Ok(raw);
    }
    std::fs::read_to_string(file)
        .with_context(|| format!("failed to read JSONL file {}", file.display()))
}

fn parse_line(line: &str) -> Result<ImportChunk> {
    let chunk: ImportChunk = serde_json::from_str(line).context("invalid JSON")?;
    if matches!(chunk.title.as_deref(), None | Some("")) {
        bail!("missing title");
    }
    Ok(chunk)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jsonl_chunks_receive_defaults_and_preserve_metadata() {
        // Given minimal and fully specified JSONL records
        // When both records are parsed
        let minimal = parse_line(r#"{"title":"First"}"#).unwrap();
        let full =
            parse_line(r#"{"title":"Second","content":"Body","type":"guide","tags":["docs"]}"#)
                .unwrap();
        // Then defaults apply only to omitted fields
        assert_eq!(minimal.content, "");
        assert_eq!(minimal.chunk_type, "note");
        assert_eq!(full.content, "Body");
        assert_eq!(full.chunk_type, "guide");
        assert_eq!(full.tags, ["docs"]);
    }

    #[test]
    fn jsonl_chunks_require_a_nonempty_title() {
        // Given invalid JSON and a record without a title
        // When both lines are parsed
        let malformed = parse_line("{");
        let missing = parse_line(r#"{"content":"Body"}"#);
        // Then each failure identifies its cause
        assert!(malformed.unwrap_err().to_string().contains("invalid JSON"));
        assert!(missing.unwrap_err().to_string().contains("missing title"));
    }
}
