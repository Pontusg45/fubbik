use std::collections::HashMap;
use std::io::{self, BufRead, BufReader, Write};
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::client::Client;
use crate::output::{self, CommandOutput, OutputMode};

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
    line: usize,
    id: String,
    title: String,
}

#[derive(Serialize)]
struct ImportError {
    line: usize,
    error: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct ReportEntry {
    line: usize,
    fingerprint: String,
    id: String,
    title: String,
}

fn default_type() -> String {
    "note".into()
}
fn fingerprint(line: &str) -> String {
    format!("{:x}", Sha256::digest(line.as_bytes()))
}

fn read_report(path: &Path) -> Result<HashMap<usize, ReportEntry>> {
    let file = std::fs::File::open(path)
        .with_context(|| format!("could not open resume report {}", path.display()))?;
    let mut entries = HashMap::new();
    for (index, line) in BufReader::new(file).lines().enumerate() {
        let line = line.with_context(|| format!("could not read report line {}", index + 1))?;
        let entry: ReportEntry = serde_json::from_str(&line)
            .with_context(|| format!("invalid report line {}", index + 1))?;
        entries.insert(entry.line, entry);
    }
    Ok(entries)
}

pub async fn run(
    client: &Client,
    file: &Path,
    dry_run: bool,
    report: Option<&Path>,
    resume: Option<&Path>,
    space: Option<&str>,
    mode: OutputMode,
) -> Result<()> {
    let mut source: Box<dyn BufRead> = if file == Path::new("-") {
        if resume.is_some() {
            bail!("--resume requires a file input");
        }
        Box::new(BufReader::new(io::stdin()))
    } else {
        Box::new(BufReader::new(std::fs::File::open(file).with_context(
            || format!("failed to open JSONL file {}", file.display()),
        )?))
    };
    let previous = resume.map(read_report).transpose()?.unwrap_or_default();
    let report_path = report.or(resume);
    let mut report_file = if dry_run {
        None
    } else if let Some(path) = report_path {
        let mut options = std::fs::OpenOptions::new();
        options.write(true);
        if resume.is_some() {
            options.append(true);
        } else {
            options.create_new(true);
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        Some(
            options
                .open(path)
                .with_context(|| format!("could not open report {}", path.display()))?,
        )
    } else {
        None
    };
    let spaces = if dry_run {
        Vec::new()
    } else {
        client
            .resolve_space(space)
            .await?
            .into_iter()
            .collect::<Vec<_>>()
    };
    let mut added = Vec::new();
    let mut errors = Vec::new();
    let mut skipped = 0usize;
    let mut would_add = 0usize;
    let mut line = String::new();
    let mut line_number = 0usize;
    loop {
        line.clear();
        if source
            .read_line(&mut line)
            .context("failed to read JSONL input")?
            == 0
        {
            break;
        }
        line_number += 1;
        if line.trim().is_empty() {
            continue;
        }
        let hash = fingerprint(line.trim_end_matches(['\r', '\n']));
        if let Some(previous) = previous.get(&line_number) {
            if previous.fingerprint != hash {
                bail!("input line {line_number} differs from its successful resume record");
            }
            skipped += 1;
            continue;
        }
        let parsed = match parse_line(&line) {
            Ok(chunk) => chunk,
            Err(error) => {
                errors.push(ImportError {
                    line: line_number,
                    error: error.to_string(),
                });
                continue;
            }
        };
        if dry_run {
            would_add += 1;
            continue;
        }
        match client
            .create_chunk(
                parsed.title.as_deref().expect("validated title"),
                &parsed.content,
                &parsed.chunk_type,
                &parsed.tags,
                &spaces,
            )
            .await
        {
            Ok(chunk) => {
                let entry = ReportEntry {
                    line: line_number,
                    fingerprint: hash,
                    id: chunk.id.clone(),
                    title: chunk.title.clone(),
                };
                if let Some(output) = &mut report_file {
                    serde_json::to_writer(&mut *output, &entry)?;
                    output.write_all(b"\n")?;
                    output.flush()?;
                }
                added.push(AddedChunk {
                    line: line_number,
                    id: chunk.id,
                    title: chunk.title,
                });
            }
            Err(error) => errors.push(ImportError {
                line: line_number,
                error: error.to_string(),
            }),
        }
    }
    let count = if dry_run { would_add } else { added.len() };
    let human = format!(
        "{} {count} chunk(s), skipped {skipped}",
        if dry_run { "Would add" } else { "Added" }
    );
    let human = if errors.is_empty() {
        human
    } else {
        format!(
            "{human}\n{} error(s):\n{}",
            errors.len(),
            errors
                .iter()
                .map(|error| format!("  Line {}: {}", error.line, error.error))
                .collect::<Vec<_>>()
                .join("\n")
        )
    };
    let quiet = added.iter().map(|chunk| chunk.id.clone()).collect();
    output::render(
        mode,
        CommandOutput {
            data: serde_json::json!({"added": added, "errors": errors, "skipped": skipped, "wouldAdd": would_add, "dryRun": dry_run}),
            human,
            quiet,
        },
    )?;
    if !errors.is_empty() {
        bail!("{} JSONL line(s) could not be imported", errors.len());
    }
    Ok(())
}

fn parse_line(line: &str) -> Result<ImportChunk> {
    let chunk: ImportChunk = serde_json::from_str(line).context("invalid JSON")?;
    if chunk
        .title
        .as_deref()
        .is_none_or(|title| title.trim().is_empty())
    {
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

    #[test]
    fn resume_report_records_successes_by_line_and_fingerprint() {
        // Given a report with a successful first line
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("report.jsonl");
        let entry = ReportEntry {
            line: 1,
            fingerprint: fingerprint(r#"{"title":"First"}"#),
            id: "chunk-1".into(),
            title: "First".into(),
        };
        std::fs::write(
            &path,
            format!("{}\n", serde_json::to_string(&entry).unwrap()),
        )
        .unwrap();
        // When the report is loaded
        let previous = read_report(&path).unwrap();
        // Then a matching input line can be skipped safely
        assert_eq!(
            previous[&1].fingerprint,
            fingerprint(r#"{"title":"First"}"#)
        );
    }

    #[tokio::test]
    async fn dry_run_validates_without_creating_chunks() {
        // Given a JSONL file with one valid and one invalid record
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("chunks.jsonl");
        std::fs::write(
            &input,
            "{\"title\":\"First\"}\n{\"content\":\"missing title\"}\n",
        )
        .unwrap();
        let server = wiremock::MockServer::start().await;
        // When the import is run as a dry run
        let result = run(
            &Client::new(server.uri()),
            &input,
            true,
            None,
            None,
            None,
            OutputMode::Quiet,
        )
        .await;
        // Then validation fails and no requests are sent
        assert!(result.is_err());
        assert!(server.received_requests().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn successful_lines_are_recorded_and_skipped_on_resume() {
        // Given a two-line input and an endpoint that accepts both chunks
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("chunks.jsonl");
        let report = dir.path().join("report.jsonl");
        std::fs::write(&input, "{\"title\":\"First\"}\n{\"title\":\"Second\"}\n").unwrap();
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .and(wiremock::matchers::path("/api/chunks"))
            .respond_with(|request: &wiremock::Request| {
                let body: serde_json::Value = serde_json::from_slice(&request.body).unwrap();
                wiremock::ResponseTemplate::new(201).set_body_json(serde_json::json!({
                    "id": body["title"].as_str().unwrap(), "title": body["title"],
                    "type": "note", "content": "", "updatedAt": "2026-01-01"
                }))
            })
            .expect(2)
            .mount(&server)
            .await;
        // When the import runs and then resumes from its report
        let client = Client::new(server.uri());
        run(
            &client,
            &input,
            false,
            Some(&report),
            None,
            None,
            OutputMode::Quiet,
        )
        .await
        .unwrap();
        run(
            &client,
            &input,
            false,
            None,
            Some(&report),
            None,
            OutputMode::Quiet,
        )
        .await
        .unwrap();
        // Then each chunk was sent once and both successes are recorded
        server.verify().await;
        assert_eq!(read_report(&report).unwrap().len(), 2);
    }
}
