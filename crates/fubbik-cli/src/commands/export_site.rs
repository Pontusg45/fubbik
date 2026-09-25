use std::collections::BTreeSet;
use std::path::Path;

use anyhow::{Context, Result, bail};

use crate::client::Client;
use crate::output::{self, OutputMode};

const CSS: &str = "*{box-sizing:border-box}body{font-family:system-ui,sans-serif;line-height:1.6;color:#172033;background:#fafafa;max-width:900px;margin:auto;padding:2rem 1rem}a{color:#2563eb;text-decoration:none}a:hover{text-decoration:underline}.header{display:flex;justify-content:space-between;align-items:center;border-bottom:1px solid #ddd;padding-bottom:1rem;margin-bottom:1.5rem}.search{width:100%;padding:.7rem;margin-bottom:1rem}.filters{display:flex;gap:.5rem;flex-wrap:wrap;margin-bottom:1rem}.filter-btn,.badge{border:1px solid #ddd;border-radius:999px;padding:.2rem .6rem;background:white}.chunk-card{border:1px solid #ddd;border-radius:8px;padding:1rem;margin-bottom:.75rem;background:white}.meta{color:#667085;font-size:.85rem}.content{margin-top:1rem}pre{background:#172033;color:#e2e8f0;padding:1rem;overflow:auto}.rationale{border-left:3px solid #22c55e;background:#f0fdf4;padding:.75rem 1rem}";

pub async fn run(
    client: &Client,
    output_dir: &Path,
    space: Option<&str>,
    mode: OutputMode,
) -> Result<()> {
    let space_id = client.resolve_space(space).await?;
    let chunks = super::export::load_chunks(client, space_id.as_deref()).await?;
    if chunks.is_empty() {
        bail!("no chunks found");
    }
    std::fs::create_dir_all(output_dir)
        .with_context(|| format!("could not create {}", output_dir.display()))?;
    std::fs::write(output_dir.join("index.html"), index_page(&chunks))?;
    for chunk in &chunks {
        let id = chunk["id"].as_str().context("export chunk omitted id")?;
        std::fs::write(
            output_dir.join(format!("chunk-{id}.html")),
            chunk_page(chunk),
        )?;
    }
    let result = serde_json::json!({"output": output_dir, "pages": chunks.len() + 1});
    match mode {
        OutputMode::Json => output::json(&result),
        OutputMode::Quiet => {
            println!("{}", output_dir.display());
            Ok(())
        }
        OutputMode::Human => {
            println!(
                "Generated {} pages to {}/",
                chunks.len() + 1,
                output_dir.display()
            );
            Ok(())
        }
    }
}

fn index_page(chunks: &[serde_json::Value]) -> String {
    let types = chunks
        .iter()
        .filter_map(|chunk| chunk["type"].as_str())
        .collect::<BTreeSet<_>>();
    let buttons = types
        .into_iter()
        .map(|kind| {
            format!(
                "<button class=\"filter-btn\" data-filter=\"{}\">{}</button>",
                escape(kind),
                escape(kind)
            )
        })
        .collect::<String>();
    let cards = chunks.iter().map(card).collect::<String>();
    format!(
        r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>fubbik Knowledge Base</title><style>{CSS}</style></head><body><div class="header"><h1>fubbik Knowledge Base</h1><span class="meta">{} chunks</span></div><input class="search" placeholder="Search chunks..."><div class="filters"><button class="filter-btn" data-filter="">All</button>{buttons}</div><div id="chunks">{cards}</div><script>let type='';const search=document.querySelector('.search');function apply(){{const q=search.value.toLowerCase();document.querySelectorAll('.chunk-card').forEach(c=>c.hidden=!((!type||c.dataset.type===type)&&(!q||c.textContent.toLowerCase().includes(q))))}}search.addEventListener('input',apply);document.querySelectorAll('.filter-btn').forEach(b=>b.addEventListener('click',()=>{{type=b.dataset.filter;apply()}}));</script></body></html>"#,
        chunks.len()
    )
}

fn card(chunk: &serde_json::Value) -> String {
    let content = chunk["content"].as_str().unwrap_or("");
    let preview = content.chars().take(150).collect::<String>();
    let suffix = if content.chars().count() > 150 {
        "..."
    } else {
        ""
    };
    format!(
        "<article class=\"chunk-card\" data-type=\"{}\"><h3><a href=\"chunk-{}.html\">{}</a></h3><p>{}{suffix}</p><div class=\"meta\">{}</div></article>",
        escape(chunk["type"].as_str().unwrap_or("note")),
        escape(chunk["id"].as_str().unwrap_or("")),
        escape(chunk["title"].as_str().unwrap_or("Untitled")),
        escape(&preview.replace('\n', " ")),
        tag_badges(chunk)
    )
}

fn chunk_page(chunk: &serde_json::Value) -> String {
    let title = escape(chunk["title"].as_str().unwrap_or("Untitled"));
    let mut extras = String::new();
    if let Some(rationale) = chunk["rationale"].as_str() {
        extras.push_str(&format!(
            "<div class=\"rationale\"><strong>Rationale:</strong> {}</div>",
            escape(rationale)
        ));
    }
    if let Some(alternatives) = chunk["alternatives"]
        .as_array()
        .filter(|items| !items.is_empty())
    {
        extras.push_str("<h2>Alternatives Considered</h2><ul>");
        for alternative in alternatives.iter().filter_map(serde_json::Value::as_str) {
            extras.push_str(&format!("<li>{}</li>", escape(alternative)));
        }
        extras.push_str("</ul>");
    }
    if let Some(consequences) = chunk["consequences"].as_str() {
        extras.push_str(&format!(
            "<h2>Consequences</h2><p>{}</p>",
            escape(consequences)
        ));
    }
    format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>{title} — fubbik</title><style>{CSS}</style></head><body><div class=\"header\"><a href=\"index.html\">&larr; Back to index</a><div>{}</div></div><h1>{title}</h1><div class=\"meta\">Updated: {}</div><div class=\"content\">{}</div>{extras}</body></html>",
        tag_badges(chunk),
        escape(chunk["updatedAt"].as_str().unwrap_or("")),
        markdown_html(chunk["content"].as_str().unwrap_or(""))
    )
}

fn tag_badges(chunk: &serde_json::Value) -> String {
    chunk["tags"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|tag| tag.as_str().or_else(|| tag["name"].as_str()))
        .map(|tag| format!("<span class=\"badge\">{}</span> ", escape(tag)))
        .collect()
}

fn markdown_html(markdown: &str) -> String {
    markdown
        .lines()
        .map(|line| {
            let line = escape(line);
            if let Some(value) = line.strip_prefix("### ") {
                format!("<h3>{value}</h3>")
            } else if let Some(value) = line.strip_prefix("## ") {
                format!("<h2>{value}</h2>")
            } else if let Some(value) = line.strip_prefix("# ") {
                format!("<h1>{value}</h1>")
            } else if let Some(value) = line.strip_prefix("- ") {
                format!("<li>{value}</li>")
            } else if line.is_empty() {
                String::new()
            } else {
                format!("<p>{line}</p>")
            }
        })
        .collect()
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_pages_escape_chunk_content_and_include_metadata() {
        // Given a chunk containing HTML-like user content and decision metadata
        let chunk = serde_json::json!({"id":"c1","title":"<Title>","type":"decision","content":"# Safe\n<script>x</script>","updatedAt":"2026-01-01","tags":[{"name":"security"}],"rationale":"Because","alternatives":["Other"],"consequences":"Safer"});
        // When index and detail pages are generated
        let index = index_page(std::slice::from_ref(&chunk));
        let detail = chunk_page(&chunk);
        // Then navigation metadata remains and raw HTML is escaped
        assert!(index.contains("chunk-c1.html"));
        assert!(detail.contains("&lt;script&gt;x&lt;/script&gt;"));
        assert!(detail.contains("Rationale:"));
        assert!(!detail.contains("<script>x</script>"));
    }
}
