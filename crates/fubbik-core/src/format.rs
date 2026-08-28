//! Chunk-to-markdown formatting.
//!
//! Ports `packages/api/src/context/utils.ts:108-126` (`formatChunkText`).
use crate::score::ScoredChunk;

fn title_case(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

pub fn format_chunk_text(chunk: &ScoredChunk) -> String {
    let type_label = match chunk.chunk_type.as_str() {
        "document" => "Architecture".to_string(),
        "convention" => "Convention".to_string(),
        "note" => "Note".to_string(),
        other => title_case(other),
    };

    let mut parts = vec![format!("## {type_label}: {}", chunk.title)];
    // Node pushes content only when truthy, so an empty string is omitted
    // rather than contributing a blank line.
    if !chunk.content.is_empty() {
        parts.push(chunk.content.clone());
    }
    if let Some(rationale) = &chunk.rationale {
        parts.push(format!("**Rationale:** {rationale}"));
    }
    parts.join("\n")
}

/// A scored chunk plus the enrichment the formatter annotates with.
/// Ports `packages/api/src/context/formatter.ts:3-7`. Node's
/// `ChunkWithMetadata extends ScoredChunk` — this port composes instead of
/// flattening, so the wire shape is reproduced with `#[serde(flatten)]`
/// rather than duplicating `ScoredChunk`'s fields here.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChunkWithMetadata {
    #[serde(flatten)]
    pub chunk: ScoredChunk,
    pub health_score: i64,
    pub is_stale: bool,
    pub has_pending_proposal: bool,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ContextSection {
    pub title: String,
    pub chunks: Vec<ChunkWithMetadata>,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StructuredContext {
    pub sections: Vec<ContextSection>,
    pub total_chunks: usize,
}

/// Ports `formatter.ts:20-35`. The `convention` tag is the single case
/// where a tag rather than the type decides the section.
fn section_title(c: &ChunkWithMetadata) -> String {
    if c.chunk.chunk_type == "note" && c.chunk.tags.iter().any(|t| t == "convention") {
        return "Conventions".to_string();
    }
    match c.chunk.chunk_type.as_str() {
        "note" => "Notes".to_string(),
        "document" => "Architecture".to_string(),
        "reference" => "API Reference".to_string(),
        "schema" => "Schemas".to_string(),
        "checklist" => "Checklists".to_string(),
        other => title_case(other),
    }
}

/// Groups chunks into sections, preserving first-seen section order.
///
/// Node builds a `Map` and iterates its entries, and JS `Map` iteration is
/// insertion-ordered — so section order follows the order sections were
/// first encountered, not alphabetical or type order. An `IndexMap`-style
/// `Vec` scan reproduces that without a new dependency.
pub fn format_structured(chunks: Vec<ChunkWithMetadata>) -> StructuredContext {
    let total_chunks = chunks.len();
    let mut sections: Vec<ContextSection> = Vec::new();

    for c in chunks {
        let title = section_title(&c);
        match sections.iter_mut().find(|s| s.title == title) {
            Some(section) => section.chunks.push(c),
            None => sections.push(ContextSection {
                title,
                chunks: vec![c],
            }),
        }
    }

    StructuredContext {
        sections,
        total_chunks,
    }
}

/// Ports `formatStructuredMarkdown` (`formatter.ts:96-115`). Not part of
/// Task 5's interface list — added here (rather than in `fubbik-api`)
/// because it's the direct sibling of [`format_structured`], which already
/// lives in this module, and all three `/api/context/*` routes need it for
/// their `structured-md` (default) response.
///
/// Trims trailing whitespace at the end, matching Node's `.join("\n").trimEnd()`.
pub fn format_structured_markdown(ctx: &StructuredContext) -> String {
    let mut lines: Vec<String> = vec!["# Project Context".to_string(), String::new()];

    for section in &ctx.sections {
        lines.push(format!("## {}", section.title));
        lines.push(String::new());
        for c in &section.chunks {
            let mut flags: Vec<&str> = Vec::new();
            if c.is_stale {
                flags.push("⚠ STALE");
            }
            if c.has_pending_proposal {
                flags.push("⚠ PENDING PROPOSAL");
            }
            let flag_str = if flags.is_empty() {
                String::new()
            } else {
                format!(" {}", flags.join(" "))
            };
            lines.push(format!(
                "### {} [health: {}]{flag_str}",
                c.chunk.title, c.health_score
            ));
            if !c.chunk.content.is_empty() {
                lines.push(String::new());
                lines.push(c.chunk.content.clone());
            }
            if let Some(rationale) = &c.chunk.rationale {
                lines.push(String::new());
                lines.push(format!("**Rationale:** {rationale}"));
            }
            lines.push(String::new());
        }
    }

    lines.join("\n").trim_end().to_string()
}

/// A behaviour-matrix rule linked to a file via `behavior_cell_code`,
/// surfaced by `GET /api/context/for-file`'s "governing behaviours"
/// section. Ports Node's `GoverningBehavior` interface
/// (`packages/api/src/context/formatter.ts:57-69`) field for field —
/// `code_ref` serialises as `ref`, matching Node's own field name, which
/// this port cannot use directly since `ref` is a Rust keyword.
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GoverningBehavior {
    pub rule_id: String,
    pub rule_title: String,
    pub description: Option<String>,
    pub rationale: Option<String>,
    pub counterexample: Option<String>,
    pub matrix_id: String,
    pub matrix_name: String,
    pub layer: String,
    pub dimension_name: String,
    pub kind: String,
    #[serde(rename = "ref")]
    pub code_ref: String,
}

/// Ports `formatBehaviorsMarkdown` (`formatter.ts:75-90`) verbatim,
/// including its "## Behaviors governing this file" heading and each
/// rule's `- [layer/matrixName] title — description` line, with an
/// indented counterexample line when present.
///
/// **Returns an empty string for an empty list, deliberately, not an
/// `Option<String>`** — Node's own doc comment says why: "so callers can
/// append unconditionally". `context_for_file::routes::for_file` relies on
/// that contract directly: it only inserts the blank-line separator when
/// this string is non-empty, so an `Option` here would just move the same
/// emptiness check into every caller instead of answering it once.
///
/// De-duplicates by `rule_id` before rendering — a rule can be linked to
/// the same file through more than one `behavior_cell_code` row (e.g. both
/// a `file` and a `symbol` link), and Node's `Set<string>` guard
/// (`formatter.ts:80-83`) ensures each rule appears once in the markdown
/// even though the caller's raw `behaviors` list (surfaced unfiltered in
/// the `structured-json` format's own `behaviors` field) may still contain
/// duplicates.
pub fn format_behaviors_markdown(behaviors: &[GoverningBehavior]) -> String {
    if behaviors.is_empty() {
        return String::new();
    }

    let mut lines: Vec<String> = vec![
        "## Behaviors governing this file".to_string(),
        String::new(),
    ];

    let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for b in behaviors {
        if !seen.insert(b.rule_id.as_str()) {
            continue;
        }

        let desc = b
            .description
            .as_deref()
            .map(|d| format!(" — {d}"))
            .unwrap_or_default();
        lines.push(format!(
            "- [{}/{}] {}{desc}",
            b.layer, b.matrix_name, b.rule_title
        ));
        if let Some(counterexample) = &b.counterexample {
            lines.push(format!("  ↳ counterexample: {counterexample}"));
        }
    }

    lines.join("\n").trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::score::ScoredChunk;

    fn chunk(chunk_type: &str, rationale: Option<&str>, content: &str) -> ScoredChunk {
        ScoredChunk {
            id: "i".into(),
            title: "Title".into(),
            content: content.into(),
            chunk_type: chunk_type.into(),
            rationale: rationale.map(str::to_string),
            tags: vec![],
            score: 0.0,
        }
    }

    #[test]
    fn document_becomes_architecture() {
        assert!(
            format_chunk_text(&chunk("document", None, "body"))
                .starts_with("## Architecture: Title")
        );
    }

    #[test]
    fn note_becomes_note_and_convention_becomes_convention() {
        assert!(format_chunk_text(&chunk("note", None, "body")).starts_with("## Note: Title"));
        assert!(
            format_chunk_text(&chunk("convention", None, "body"))
                .starts_with("## Convention: Title")
        );
    }

    /// Unknown types are title-cased rather than passed through, matching
    /// Node's `charAt(0).toUpperCase() + slice(1)`.
    #[test]
    fn unknown_type_is_title_cased() {
        assert!(
            format_chunk_text(&chunk("runbook", None, "body")).starts_with("## Runbook: Title")
        );
    }

    #[test]
    fn rationale_is_appended_with_its_label() {
        let out = format_chunk_text(&chunk("note", Some("because"), "body"));
        assert_eq!(out, "## Note: Title\nbody\n**Rationale:** because");
    }

    /// Node pushes content only when it is truthy, so an empty string is
    /// omitted rather than producing a blank line.
    #[test]
    fn empty_content_is_omitted_not_blank() {
        assert_eq!(
            format_chunk_text(&chunk("note", None, "")),
            "## Note: Title"
        );
    }

    fn with_meta(chunk_type: &str, tags: &[&str]) -> ChunkWithMetadata {
        ChunkWithMetadata {
            chunk: ScoredChunk {
                id: "i".into(),
                title: "T".into(),
                content: "c".into(),
                chunk_type: chunk_type.into(),
                rationale: None,
                tags: tags.iter().map(|t| t.to_string()).collect(),
                score: 0.0,
            },
            health_score: 50,
            is_stale: false,
            has_pending_proposal: false,
        }
    }

    #[test]
    fn types_map_to_their_section_titles() {
        let out = format_structured(vec![
            with_meta("note", &[]),
            with_meta("document", &[]),
            with_meta("reference", &[]),
            with_meta("schema", &[]),
            with_meta("checklist", &[]),
        ]);
        let titles: Vec<&str> = out.sections.iter().map(|s| s.title.as_str()).collect();
        assert!(titles.contains(&"Notes"));
        assert!(titles.contains(&"Architecture"));
        assert!(titles.contains(&"API Reference"));
        assert!(titles.contains(&"Schemas"));
        assert!(titles.contains(&"Checklists"));
    }

    /// A note tagged `convention` is pulled out of Notes into its own
    /// section — the one case where the tag, not the type, decides.
    #[test]
    fn a_note_tagged_convention_becomes_its_own_section() {
        let out = format_structured(vec![with_meta("note", &["convention"])]);
        assert_eq!(out.sections.len(), 1);
        assert_eq!(out.sections[0].title, "Conventions");
    }

    #[test]
    fn a_note_without_the_tag_stays_in_notes() {
        let out = format_structured(vec![with_meta("note", &["other"])]);
        assert_eq!(out.sections[0].title, "Notes");
    }

    #[test]
    fn chunks_of_one_type_group_into_a_single_section() {
        let out = format_structured(vec![with_meta("note", &[]), with_meta("note", &[])]);
        assert_eq!(out.sections.len(), 1);
        assert_eq!(out.sections[0].chunks.len(), 2);
        assert_eq!(out.total_chunks, 2);
    }

    #[test]
    fn unknown_type_gets_a_title_cased_section() {
        let out = format_structured(vec![with_meta("runbook", &[])]);
        assert_eq!(out.sections[0].title, "Runbook");
    }

    /// Node builds a `Map` and iterates its entries; JS `Map` iteration is
    /// insertion-ordered, so sections must appear in the order they were
    /// first encountered in the input, not alphabetically or by type. This
    /// is parity with Node, not an arbitrary choice. The repeated
    /// `checklist` at the end confirms that re-encountering a type appends
    /// to the existing section rather than creating a new one or moving it.
    #[test]
    fn sections_appear_in_first_encounter_order() {
        let out = format_structured(vec![
            with_meta("checklist", &[]),
            with_meta("note", &[]),
            with_meta("document", &[]),
            with_meta("checklist", &[]), // repeat — must not move the section
        ]);
        let titles: Vec<&str> = out.sections.iter().map(|s| s.title.as_str()).collect();
        assert_eq!(titles, vec!["Checklists", "Notes", "Architecture"]);
    }

    #[test]
    fn markdown_includes_health_flags_and_rationale() {
        let mut meta = with_meta("note", &[]);
        meta.chunk.title = "Widget".into();
        meta.chunk.content = "Body text".into();
        meta.chunk.rationale = Some("because reasons".into());
        meta.health_score = 42;
        meta.is_stale = true;
        meta.has_pending_proposal = true;

        let out = format_structured_markdown(&format_structured(vec![meta]));

        assert!(out.starts_with("# Project Context"));
        assert!(out.contains("## Notes"));
        assert!(out.contains("### Widget [health: 42] ⚠ STALE ⚠ PENDING PROPOSAL"));
        assert!(out.contains("Body text"));
        assert!(out.contains("**Rationale:** because reasons"));
    }

    #[test]
    fn markdown_trims_trailing_whitespace() {
        let out = format_structured_markdown(&format_structured(vec![with_meta("note", &[])]));
        assert_eq!(out, out.trim_end());
        assert!(!out.ends_with('\n'));
    }
}
