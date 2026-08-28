//! `GET /api/chunks/export/claude-md`.
//!
//! Ports `packages/api/src/context-export/claude-md.ts` (190 LOC):
//! tag-based export of chunks (grouped into four fixed sections by type),
//! followed by a Requirements section and an Active Plans section, joined
//! with blank lines and truncated to a token budget that defaults to
//! 32000.
//!
//! **The budget rebuild drops Requirements and Active Plans entirely, not
//! just the excess.** Node's truncation branch (`claude-md.ts:138-186`)
//! builds a brand-new `budgetedParts` array from scratch, containing only
//! the header and whatever chunk sections/entries fit — it never revisits
//! the requirements or plans text at all. This looks like an oversight,
//! but this is a straight port, and the task brief's opening line is
//! explicit: ask before guessing, don't "fix" a asymmetry Node itself
//! ships. Confirmed by rereading `claude-md.ts` line by line rather than
//! summarising it.
use std::collections::HashMap;

use fubbik_core::error::AppResult;
use fubbik_core::tokens::estimate_tokens;
use fubbik_db::repo::chunk::{self, Chunk};
use fubbik_db::repo::{plan, requirement};
use serde::Serialize;
use sqlx::PgPool;
use utoipa::ToSchema;

/// Node's default when `maxTokens` is omitted (`claude-md.ts:134`).
pub const DEFAULT_MAX_TOKENS: usize = 32000;

/// Node's default tag name when `tag` is omitted (`claude-md.ts:41`).
const DEFAULT_TAG: &str = "claude-context";

pub struct ClaudeMdParams<'a> {
    pub space_id: Option<&'a str>,
    pub tag: Option<&'a str>,
    pub max_tokens: usize,
}

/// Matches Node's `{content, chunks}` response (`claude-md.ts:188`) —
/// `chunks` is the tagged-chunk *count*, not the chunk list itself.
#[derive(Debug, Serialize, ToSchema)]
pub struct ClaudeMdResponse {
    pub content: String,
    pub chunks: usize,
}

/// Fixed section order the final document always walks in, regardless of
/// which order sections were first populated in — matches Node's
/// `sectionOrder` array, reused verbatim by both the unbudgeted assembly
/// (`claude-md.ts:65`) and the budget rebuild (`claude-md.ts:153`).
const SECTION_ORDER: [&str; 4] = ["Conventions", "Architecture", "References", "Other"];

/// Ports `sectionLabel` (`claude-md.ts:29-31`): `note` -> Conventions,
/// `document` -> Architecture, `reference` -> References, everything else
/// -> Other. Unlike `context::formatter`'s `section_title`, there is no
/// tag-based special case here — Node's `TYPE_SECTIONS` map is
/// type-only.
fn section_label(chunk_type: &str) -> &'static str {
    match chunk_type {
        "note" => "Conventions",
        "document" => "Architecture",
        "reference" => "References",
        _ => "Other",
    }
}

/// Ports `formatChunkEntry` (`claude-md.ts:33-38`): a `### Title` heading,
/// then content (only when non-empty), then a `**Rationale:**` line (only
/// when present), each joined by a blank line.
fn format_chunk_entry(c: &Chunk) -> String {
    let mut parts = vec![format!("### {}", c.title)];
    if !c.content.is_empty() {
        parts.push(c.content.clone());
    }
    if let Some(r) = &c.rationale {
        parts.push(format!("**Rationale:** {r}"));
    }
    parts.join("\n\n")
}

/// Groups chunks by `section_label`. A plain `HashMap` is safe here — unlike
/// `context::formatter::format_structured`, the caller always walks
/// `SECTION_ORDER` to read this back out, so `HashMap`'s unordered
/// iteration never leaks into the output order.
fn group_by_section(chunks: &[Chunk]) -> HashMap<&'static str, Vec<&Chunk>> {
    let mut sections: HashMap<&'static str, Vec<&Chunk>> = HashMap::new();
    for c in chunks {
        sections
            .entry(section_label(&c.chunk_type))
            .or_default()
            .push(c);
    }
    sections
}

/// Ports `generateClaudeMd` (`claude-md.ts:40-190`).
pub async fn generate_claude_md(
    pool: &PgPool,
    user_id: &str,
    params: ClaudeMdParams<'_>,
) -> AppResult<ClaudeMdResponse> {
    let tag_name = params.tag.unwrap_or(DEFAULT_TAG).to_string();

    // `chunk::list`'s `tags` filter is OR-across-names, but a single-name
    // list degenerates to exactly Node's `listChunksByTag`'s single-tag
    // membership check (`tag-new.ts:179-208`) — including its space
    // semantics (a chunk in the named space, or in no space at all) and
    // its `ORDER BY chunk.title` (`Sort::Alpha`).
    let chunks = chunk::list(
        pool,
        user_id,
        &chunk::ListParams {
            tags: Some(vec![tag_name.clone()]),
            space_id: params.space_id.map(str::to_string),
            sort: chunk::Sort::Alpha,
            limit: 100,
            ..Default::default()
        },
    )
    .await?;

    let mut parts: Vec<String> = vec!["# Project Context\n".to_string()];

    if chunks.is_empty() {
        parts.push(format!("No chunks found with tag \"{tag_name}\".\n"));
    } else {
        let sections = group_by_section(&chunks);
        for section_name in SECTION_ORDER {
            let Some(group) = sections.get(section_name) else {
                continue;
            };
            if group.is_empty() {
                continue;
            }
            parts.push(format!("## {section_name}\n"));
            for c in group {
                parts.push(format_chunk_entry(c));
            }
        }
    }

    // ── Requirements section ──
    let requirements = requirement::list(
        pool,
        user_id,
        &requirement::ListParams {
            space_id: params.space_id,
            use_case_id: None,
            status: None,
            priority: None,
            origin: None,
            review_status: None,
            search: None,
            limit: 50,
            offset: 0,
        },
    )
    .await?;

    if !requirements.is_empty() {
        parts.push("## Requirements\n".to_string());

        // failing < untested < passing < (anything else), matching Node's
        // `statusOrder` map with a `?? 3` fallback (`claude-md.ts:87`).
        let status_rank = |s: &str| match s {
            "failing" => 0,
            "untested" => 1,
            "passing" => 2,
            _ => 3,
        };
        let mut sorted = requirements.clone();
        sorted.sort_by_key(|r| status_rank(&r.status));

        for req in &sorted {
            let marker = if req.status == "failing" || req.status == "untested" {
                " <!-- ACTION NEEDED -->"
            } else {
                ""
            };
            let priority = req
                .priority
                .as_deref()
                .map(|p| format!(" [{p}]"))
                .unwrap_or_default();
            parts.push(format!(
                "### {}{priority} — {}{marker}",
                req.title, req.status
            ));

            if !req.steps.0.is_empty() {
                let steps_text = req
                    .steps
                    .0
                    .iter()
                    .map(|s| format!("- **{}** {}", s.keyword, s.text))
                    .collect::<Vec<_>>()
                    .join("\n");
                parts.push(steps_text);
            }

            let linked_chunks = requirement::get_chunks(pool, user_id, &req.id).await?;
            if !linked_chunks.is_empty() {
                let chunk_list = linked_chunks
                    .iter()
                    .map(|c| c.title.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                parts.push(format!("**Linked chunks:** {chunk_list}"));
            }
        }
    }

    // ── Active plans section ──
    let plans = plan::list(
        pool,
        user_id,
        plan::ListFilter {
            space_id: params.space_id.map(str::to_string),
            status: Some("in_progress".to_string()),
            requirement_id: None,
            include_archived: false,
        },
    )
    .await?;

    if !plans.is_empty() {
        parts.push("## Active Plans\n".to_string());

        for p in &plans {
            let tasks = plan::list_tasks(pool, user_id, &p.id).await?;
            let done = tasks.iter().filter(|t| t.status == "done").count();
            let total = tasks.len();
            let pct = if total > 0 {
                ((done as f64 / total as f64) * 100.0).round() as i64
            } else {
                0
            };

            parts.push(format!("### {} ({done}/{total} tasks — {pct}%)", p.title));

            let pending: Vec<&_> = tasks
                .iter()
                .filter(|t| t.status == "pending" || t.status == "in_progress")
                .collect();
            if !pending.is_empty() {
                let pending_text = pending
                    .iter()
                    .map(|t| format!("- [ ] {}", t.title))
                    .collect::<Vec<_>>()
                    .join("\n");
                parts.push(pending_text);
            }
        }
    }

    let mut content = parts.join("\n\n");
    let total_tokens = estimate_tokens(&content);

    if total_tokens > params.max_tokens {
        // Rebuild with budget: only the chunk sections are re-included,
        // greedily, until the budget is exhausted. Requirements and Active
        // Plans are dropped wholesale on this path — see the module doc.
        let mut budgeted_parts: Vec<String> = vec!["# Project Context\n".to_string()];
        let mut used_tokens = estimate_tokens(&budgeted_parts[0]);
        let mut omitted_chunks = 0usize;

        if !chunks.is_empty() {
            let sections = group_by_section(&chunks);
            for section_name in SECTION_ORDER {
                let Some(group) = sections.get(section_name) else {
                    continue;
                };
                if group.is_empty() {
                    continue;
                }
                let section_header = format!("## {section_name}\n");
                let section_tokens = estimate_tokens(&section_header);
                if used_tokens + section_tokens > params.max_tokens {
                    omitted_chunks += group.len();
                    continue;
                }
                budgeted_parts.push(section_header);
                used_tokens += section_tokens;

                for c in group {
                    let entry = format_chunk_entry(c);
                    let entry_tokens = estimate_tokens(&entry);
                    if used_tokens + entry_tokens > params.max_tokens {
                        omitted_chunks += 1;
                        continue;
                    }
                    budgeted_parts.push(entry);
                    used_tokens += entry_tokens;
                }
            }
        }

        if omitted_chunks > 0 {
            budgeted_parts.push(format!(
                "<!-- Truncated: {omitted_chunks} chunks omitted due to token budget. Increase maxTokens or narrow the tag filter. -->"
            ));
        }

        content = budgeted_parts.join("\n\n");
    }

    Ok(ClaudeMdResponse {
        content,
        chunks: chunks.len(),
    })
}
