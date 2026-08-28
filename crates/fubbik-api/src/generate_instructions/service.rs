//! Ports `generateInstructions` (`packages/api/src/generate-instructions/
//! service.ts:29-79`) plus its `categorizeChunks`/`formatClaude`/
//! `formatAgents`/`formatCursor` helpers, end to end.

use fubbik_core::error::{AppError, AppResult};
use fubbik_db::repo::chunk::{self, ListParams, Sort};
use fubbik_db::repo::space;
use fubbik_db::repo::tag;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use utoipa::ToSchema;

/// The `format` query param. Node's `t.Union([t.Literal("claude"),
/// t.Literal("agents"), t.Literal("cursor")])` (`routes.ts:21`) rejects
/// anything outside these three literals at the schema layer, before the
/// handler ever runs — Elysia responds `422` for an unrecognised value
/// rather than falling back to a default. This port matches that with
/// serde's own literal-enum rejection: an unknown `format` fails to
/// deserialize the query string, which `extract::Query` turns into a `400`
/// (see `unknown_format_is_rejected_or_defaults` in
/// `tests/generate_instructions.rs` for which of the two — reject vs.
/// silently default — this actually is).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum InstructionFormat {
    Claude,
    Agents,
    Cursor,
}

/// Node's `query.format ?? "claude"` (`service.ts:30`) — but since the
/// query-schema layer already rejects anything other than the three
/// literals (see [`InstructionFormat`]'s doc comment), the only way
/// `format` reaches the service as `None` is when the query param is
/// omitted entirely, exactly like Node.
const DEFAULT_FORMAT: InstructionFormat = InstructionFormat::Claude;

#[derive(Debug, Serialize, ToSchema)]
pub struct GenerateInstructionsResponse {
    pub format: &'static str,
    pub content: String,
}

struct ChunkWithTags {
    title: String,
    content: String,
    chunk_type: String,
    rationale: Option<String>,
    alternatives: Option<Vec<String>>,
    consequences: Option<String>,
    tags: Vec<String>,
}

#[derive(Default)]
struct CategorizedChunks {
    overview: Vec<usize>,
    architecture: Vec<usize>,
    conventions: Vec<usize>,
    commands: Vec<usize>,
    other: Vec<usize>,
}

/// Ports `generateInstructions` (`service.ts:29-79`).
///
/// **Ownership check is new, not a port** — see `generate_instructions::`
/// module doc comment for why. Node hands `space_id` straight to the list
/// query with no existence/ownership check at all.
pub async fn generate_instructions(
    pool: &PgPool,
    user_id: &str,
    space_id: &str,
    format: Option<InstructionFormat>,
) -> AppResult<GenerateInstructionsResponse> {
    space::find_by_id(pool, user_id, space_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Space".into()))?;

    let format = format.unwrap_or(DEFAULT_FORMAT);

    // Matches Node's `listChunksRepo({ userId, spaceId, limit: 500, offset:
    // 0 })` (`service.ts:31-35`) width exactly, via `chunk::list_internal`
    // rather than `chunk::list` — the same reasoning `context_export`
    // documents: `chunk::list`'s 100-row clamp exists for `GET
    // /api/chunks` and `POST /api/search/query` specifically, and would
    // silently narrow this endpoint's fetch to a fifth of Node's width if
    // used here instead.
    let params = ListParams {
        space_id: Some(space_id.to_string()),
        sort: Sort::Newest,
        limit: 500,
        ..Default::default()
    };
    let chunks = chunk::list_internal(pool, user_id, &params, 500).await?;

    let chunk_ids: Vec<String> = chunks.iter().map(|c| c.id.clone()).collect();
    let tag_rows = tag::tags_for_chunks(pool, user_id, &chunk_ids).await?;

    // Build a per-chunk tag list, preserving each chunk's own tag order
    // (insertion order, matching Node's `Map` + `push` accumulation in
    // `service.ts:44-49`). Iterating the *rows* to build this is fine —
    // what matters is that `chunks` (the thing we render in order) is
    // never itself produced by iterating a hash container.
    let mut tag_map: std::collections::HashMap<&str, Vec<String>> =
        std::collections::HashMap::new();
    for row in &tag_rows {
        tag_map
            .entry(row.chunk_id.as_str())
            .or_default()
            .push(row.tag_name.clone());
    }

    let enriched: Vec<ChunkWithTags> = chunks
        .iter()
        .map(|c| ChunkWithTags {
            title: c.title.clone(),
            content: c.content.clone(),
            chunk_type: c.chunk_type.clone(),
            rationale: c.rationale.clone(),
            alternatives: c.alternatives.as_ref().map(|a| a.0.clone()),
            consequences: c.consequences.clone(),
            tags: tag_map.get(c.id.as_str()).cloned().unwrap_or_default(),
        })
        .collect();

    let categorized = categorize_chunks(&enriched);

    let content = match format {
        InstructionFormat::Claude => format_claude(&enriched, &categorized),
        InstructionFormat::Agents => format_agents(&enriched, &categorized),
        InstructionFormat::Cursor => format_cursor(&enriched, &categorized),
    };

    let format_str = match format {
        InstructionFormat::Claude => "claude",
        InstructionFormat::Agents => "agents",
        InstructionFormat::Cursor => "cursor",
    };

    Ok(GenerateInstructionsResponse {
        format: format_str,
        content,
    })
}

/// Ports `categorizeChunks` (`service.ts:83-127`) exactly, including its
/// quirks:
///
/// - A chunk can land in more than one bucket (`overview` is not
///   exclusive of `architecture`/`conventions`/`commands`) — the `if`s are
///   independent, not an `if`/`else if` chain, matching Node.
/// - `other` is only reached when a chunk landed in *none* of
///   `overview`/`architecture`/`conventions`/`commands` — Node checks this
///   with `Array.prototype.includes` against the four arrays already
///   built for this chunk (`service.ts:118-124`); this port tracks the
///   same "did this chunk land anywhere yet" fact directly instead of
///   re-scanning four `Vec`s per chunk, which would also be wrong once a
///   chunk could appear twice in the same bucket (it can't here, but
///   `includes`-based dedup is not what this exists to prove).
fn categorize_chunks(chunks: &[ChunkWithTags]) -> CategorizedChunks {
    let mut result = CategorizedChunks::default();

    for (i, chunk) in chunks.iter().enumerate() {
        let lower_tags: Vec<String> = chunk.tags.iter().map(|t| t.to_lowercase()).collect();
        let lower_content = chunk.content.to_lowercase();
        let lower_title = chunk.title.to_lowercase();

        let mut landed = false;

        if lower_tags
            .iter()
            .any(|t| t == "architecture" || t == "core")
        {
            result.overview.push(i);
            landed = true;
        }

        // Node writes this as two separate `if`/`else if` branches
        // (`service.ts:99-105`) that both push the same chunk into
        // `architecture` — collapsed into one `||` condition here since
        // the two branches are behaviourally identical (clippy's
        // `if_same_then_else` flags a literal port of the two-branch
        // shape as redundant).
        if chunk.chunk_type == "document"
            && (lower_tags.iter().any(|t| t == "architecture")
                || lower_title.contains("architecture")
                || lower_title.contains("pattern"))
        {
            result.architecture.push(i);
            landed = true;
        }

        // Node: `chunk.rationale || ...` — JS truthiness, so `Some("")` must
        // not count as present.
        if chunk.rationale.as_deref().is_some_and(|r| !r.is_empty())
            || lower_tags
                .iter()
                .any(|t| t == "convention" || t == "conventions")
        {
            result.conventions.push(i);
            landed = true;
        }

        if lower_content.contains("pnpm ")
            || lower_content.contains("npm ")
            || lower_content.contains("bun ")
            || lower_tags.iter().any(|t| t == "commands" || t == "scripts")
        {
            result.commands.push(i);
            landed = true;
        }

        if !landed {
            result.other.push(i);
        }
    }

    result
}

fn push_body(sections: &mut Vec<String>, c: &ChunkWithTags, with_title: bool) {
    if with_title {
        sections.push(format!("### {}", c.title));
        sections.push(String::new());
    }
    sections.push(c.content.clone());
    sections.push(String::new());
}

/// Ports `formatClaude` (`service.ts:151-217`).
fn format_claude(chunks: &[ChunkWithTags], cat: &CategorizedChunks) -> String {
    let mut sections: Vec<String> = vec![
        "# CLAUDE.md".to_string(),
        String::new(),
        "This file provides context about the project for AI assistants.".to_string(),
        String::new(),
    ];

    if !cat.overview.is_empty() {
        sections.push("## Project Overview".to_string());
        sections.push(String::new());
        for &i in &cat.overview {
            sections.push(chunks[i].content.clone());
            sections.push(String::new());
        }
    }

    if !cat.architecture.is_empty() {
        sections.push("## Architecture".to_string());
        sections.push(String::new());
        for &i in &cat.architecture {
            push_body(&mut sections, &chunks[i], true);
        }
    }

    if !cat.conventions.is_empty() {
        sections.push("## Conventions".to_string());
        sections.push(String::new());
        for &i in &cat.conventions {
            let c = &chunks[i];
            sections.push(format!("### {}", c.title));
            sections.push(String::new());
            sections.push(c.content.clone());
            // Node: `if (c.rationale)` — truthy-checks the string, so an
            // empty-string rationale must be omitted, not just `None`.
            if let Some(r) = &c.rationale
                && !r.is_empty()
            {
                sections.push(String::new());
                sections.push(format!("**Rationale:** {r}"));
            }
            if let Some(alts) = &c.alternatives
                && !alts.is_empty()
            {
                sections.push(String::new());
                sections.push(format!("**Alternatives considered:** {}", alts.join(", ")));
            }
            if let Some(cons) = &c.consequences {
                sections.push(String::new());
                sections.push(format!("**Consequences:** {cons}"));
            }
            sections.push(String::new());
        }
    }

    if !cat.commands.is_empty() {
        sections.push("## Commands".to_string());
        sections.push(String::new());
        for &i in &cat.commands {
            push_body(&mut sections, &chunks[i], true);
        }
    }

    if !cat.other.is_empty() {
        sections.push("## Additional Context".to_string());
        sections.push(String::new());
        for &i in &cat.other {
            push_body(&mut sections, &chunks[i], true);
        }
    }

    sections.join("\n")
}

/// Ports `formatAgents` (`service.ts:219-267`).
fn format_agents(chunks: &[ChunkWithTags], cat: &CategorizedChunks) -> String {
    let mut sections: Vec<String> = vec![
        "# AGENTS.md".to_string(),
        String::new(),
        "Instructions for AI agents working on this project.".to_string(),
        String::new(),
    ];

    if !cat.conventions.is_empty() {
        sections.push("## Rules".to_string());
        sections.push(String::new());
        for &i in &cat.conventions {
            let c = &chunks[i];
            let first_line = c.content.split('\n').next().unwrap_or("");
            sections.push(format!("- **{}**: {first_line}", c.title));
            // Node: `if (c.rationale)` — same truthy-check as above.
            if let Some(r) = &c.rationale
                && !r.is_empty()
            {
                sections.push(format!("  - Rationale: {r}"));
            }
        }
        sections.push(String::new());
    }

    if !cat.architecture.is_empty() {
        sections.push("## Architecture".to_string());
        sections.push(String::new());
        for &i in &cat.architecture {
            push_body(&mut sections, &chunks[i], true);
        }
    }

    if !cat.commands.is_empty() {
        sections.push("## Available Commands".to_string());
        sections.push(String::new());
        for &i in &cat.commands {
            push_body(&mut sections, &chunks[i], true);
        }
    }

    if !cat.other.is_empty() {
        sections.push("## Additional Context".to_string());
        sections.push(String::new());
        for &i in &cat.other {
            push_body(&mut sections, &chunks[i], true);
        }
    }

    sections.join("\n")
}

/// Ports `formatCursor` (`service.ts:269-301`). Unlike the other two
/// formats, this one has no "Additional Context"/`other` section at all —
/// matching Node, which never reads `cat.other` in `formatCursor`.
fn format_cursor(chunks: &[ChunkWithTags], cat: &CategorizedChunks) -> String {
    let mut sections: Vec<String> = Vec::new();
    sections.push("# .cursorrules".to_string());
    sections.push(String::new());

    if !cat.conventions.is_empty() {
        sections.push("## Coding Conventions".to_string());
        sections.push(String::new());
        for &i in &cat.conventions {
            let c = &chunks[i];
            let first_line = c.content.split('\n').next().unwrap_or("");
            sections.push(format!("- {}: {first_line}", c.title));
        }
        sections.push(String::new());
    }

    if !cat.architecture.is_empty() {
        sections.push("## Architecture".to_string());
        sections.push(String::new());
        for &i in &cat.architecture {
            let c = &chunks[i];
            sections.push(format!("### {}", c.title));
            sections.push(c.content.clone());
            sections.push(String::new());
        }
    }

    if !cat.commands.is_empty() {
        sections.push("## Commands".to_string());
        sections.push(String::new());
        for &i in &cat.commands {
            sections.push(chunks[i].content.clone());
            sections.push(String::new());
        }
    }

    sections.join("\n")
}
