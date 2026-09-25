use std::path::Path;

use anyhow::{Context, Result};

use crate::client::Client;
use crate::output::{self, OutputMode};

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct FileStats {
    lines: usize,
    functions: usize,
    imports: usize,
    exports: usize,
    classes: usize,
}

#[derive(Debug, PartialEq, Eq, serde::Serialize)]
struct Suggestion {
    #[serde(rename = "type")]
    chunk_type: String,
    title: String,
    reason: String,
}

pub async fn run(
    client: &Client,
    path: &Path,
    space: Option<&str>,
    mode: OutputMode,
) -> Result<()> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("cannot read file: {}", path.display()))?;
    let stats = analyze(&content);
    let space = client.resolve_space(space).await?;
    let context = client
        .context_for_file(
            &path.to_string_lossy(),
            space.as_deref(),
            8000,
            "json-legacy",
        )
        .await?;
    let existing = context["chunks"].as_array().cloned().unwrap_or_default();
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("file");
    let suggestions = suggestions(file_name, &stats, &existing);
    let result = serde_json::json!({ "suggestions": suggestions, "stats": stats });
    if mode == OutputMode::Json {
        return output::json(&result);
    }
    if suggestions.is_empty() {
        println!("No suggestions — file appears well-covered");
        return Ok(());
    }
    println!(
        "{} suggestion(s) for {}:",
        suggestions.len(),
        path.display()
    );
    println!(
        "  Analysis: {} lines, {} functions, {} imports, {} exports\n",
        stats.lines, stats.functions, stats.imports, stats.exports
    );
    for suggestion in suggestions {
        println!(
            "  [{}] {}\n    {}",
            suggestion.chunk_type, suggestion.title, suggestion.reason
        );
    }
    Ok(())
}

fn analyze(content: &str) -> FileStats {
    let lines = content.split('\n').count();
    let imports = content
        .lines()
        .filter(|line| line.starts_with("import "))
        .count();
    let exports = content
        .lines()
        .filter(|line| line.starts_with("export "))
        .count();
    let classes = content
        .lines()
        .filter(|line| {
            let line = line.trim_start();
            line.starts_with("class ") || line.starts_with("export class ")
        })
        .count();
    let functions = content.match_indices("function ").count()
        + content.match_indices(" = (").count()
        + content.match_indices(" = async (").count();
    FileStats {
        lines,
        functions,
        imports,
        exports,
        classes,
    }
}

fn suggestions(
    file_name: &str,
    stats: &FileStats,
    existing: &[serde_json::Value],
) -> Vec<Suggestion> {
    let has_type = |kind: &str| existing.iter().any(|chunk| chunk["type"] == kind);
    let mut result = Vec::new();
    if existing.is_empty() {
        result.push(suggestion(
            "reference",
            format!("{file_name} Documentation"),
            "No chunks reference this file".into(),
        ));
    }
    if stats.functions > 5 && !has_type("reference") {
        result.push(suggestion(
            "reference",
            format!("{file_name} API Reference"),
            format!("{} functions found but no reference chunk", stats.functions),
        ));
    }
    if stats.exports > 3 {
        result.push(suggestion(
            "document",
            format!("{file_name} Architecture"),
            format!("{} exports suggest this is a key module", stats.exports),
        ));
    }
    if stats.lines > 200 {
        result.push(suggestion(
            "note",
            format!("{file_name} Conventions"),
            format!(
                "Large file ({} lines) may have implicit conventions",
                stats.lines
            ),
        ));
    }
    if stats.classes > 0 && !has_type("schema") {
        result.push(suggestion(
            "schema",
            format!("{file_name} Class Schema"),
            format!("{} class(es) found but no schema chunk", stats.classes),
        ));
    }
    result
}

fn suggestion(chunk_type: &str, title: String, reason: String) -> Suggestion {
    Suggestion {
        chunk_type: chunk_type.into(),
        title,
        reason,
    }
}

#[cfg(test)]
mod tests {
    use super::{analyze, suggestions};

    #[test]
    fn uncovered_complex_file_produces_reference_architecture_and_schema_suggestions() {
        // Given an uncovered source file with many functions, exports, and a class
        let source = "export class Api {}\nexport function a() {}\nexport function b() {}\nexport function c() {}\nexport function d() {}\nexport function e() {}\nexport function f() {}";
        let stats = analyze(source);

        // When suggestions are generated without existing chunks
        let suggestions = suggestions("api.ts", &stats, &[]);

        // Then documentation, API, architecture, and schema gaps are reported
        assert_eq!(suggestions.len(), 4);
        assert_eq!(suggestions[0].chunk_type, "reference");
        assert_eq!(suggestions[1].title, "api.ts API Reference");
        assert_eq!(suggestions[2].chunk_type, "document");
        assert_eq!(suggestions[3].chunk_type, "schema");
    }
}
