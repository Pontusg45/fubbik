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
}
