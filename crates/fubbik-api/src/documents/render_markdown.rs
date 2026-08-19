//! Port of `packages/api/src/documents/render-markdown.ts`.

use super::frontmatter::tags_from_path;
use super::split_markdown::MarkdownSection;

pub struct RenderOptions<'a> {
    pub title: &'a str,
    /// `None` (Node: `opts.type` falsy/absent) and `Some("document")` both
    /// suppress the `type:` frontmatter line — Node's guard is `if
    /// (opts.type && opts.type !== "document")`.
    pub doc_type: Option<&'a str>,
    pub tags: &'a [String],
    /// `Some` only when the source object is non-empty, matching the
    /// service layer's `scope && Object.keys(scope).length > 0 ? scope :
    /// undefined` pre-check (`documents::service::render_document`) — this
    /// function itself does not re-check emptiness beyond that.
    pub scope: Option<&'a serde_json::Value>,
    pub split_level: i32,
    pub sections: &'a [MarkdownSection],
    pub source_path: Option<&'a str>,
}

/// Renders a document back to markdown, matching `renderMarkdown`
/// (`packages/api/src/documents/render-markdown.ts:14-79`) line for line.
pub fn render_markdown(opts: RenderOptions) -> String {
    let mut lines: Vec<String> = Vec::new();
    let mut fm_lines: Vec<String> = Vec::new();

    fm_lines.push(format!("title: {}", opts.title));
    if let Some(t) = opts.doc_type
        && t != "document"
    {
        fm_lines.push(format!("type: {t}"));
    }

    // Exclude path-derived tags to avoid duplication on re-import.
    let path_tags: std::collections::HashSet<String> = opts
        .source_path
        .map(|p| tags_from_path(p).into_iter().collect())
        .unwrap_or_default();
    let content_tags: Vec<&String> = opts
        .tags
        .iter()
        .filter(|t| !path_tags.contains(*t))
        .collect();
    if !content_tags.is_empty() {
        fm_lines.push("tags:".to_string());
        for tag in content_tags {
            fm_lines.push(format!("  - {tag}"));
        }
    }

    if let Some(serde_json::Value::Object(map)) = opts.scope
        && !map.is_empty()
    {
        fm_lines.push("scope:".to_string());
        for (key, value) in map {
            let rendered = match value {
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            fm_lines.push(format!("  {key}: {rendered}"));
        }
    }

    lines.push("---".to_string());
    lines.extend(fm_lines);
    lines.push("---".to_string());
    lines.push(String::new());

    let prefix = "#".repeat(opts.split_level.max(0) as usize);

    for section in opts.sections {
        let is_intro = section.title.ends_with(" \u{2014} Introduction");

        if !is_intro {
            lines.push(format!("{prefix} {}", section.title));
            lines.push(String::new());
        }

        if !section.content.is_empty() {
            lines.push(section.content.clone());
            lines.push(String::new());
        }

        if let Some(r) = &section.rationale
            && !r.is_empty()
        {
            lines.push(format!("> **Rationale:** {r}"));
            lines.push(String::new());
        }

        if let Some(alts) = &section.alternatives
            && !alts.is_empty()
        {
            lines.push("> **Alternatives:**".to_string());
            for alt in alts {
                lines.push(format!("> - {alt}"));
            }
            lines.push(String::new());
        }

        if let Some(c) = &section.consequences
            && !c.is_empty()
        {
            lines.push(format!("> **Consequences:** {c}"));
            lines.push(String::new());
        }
    }

    lines.join("\n").trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::documents::split_markdown::{split_markdown, split_markdown_with_level};

    #[test]
    fn round_trips_frontmatter_through_split_render_split() {
        let md = "---\ntitle: Auth Guide\ntype: reference\ntags:\n  - security\n  - backend\n---\n\n## Setup\n\nInstall the auth library.\n\n## Configuration\n\nEdit config.json.";

        let first = split_markdown(md, "docs/auth.md");
        assert_eq!(first.title, "Auth Guide");
        assert_eq!(first.sections.len(), 2);

        let tags = vec!["security".to_string(), "backend".to_string()];
        let rendered = render_markdown(RenderOptions {
            title: &first.title,
            doc_type: Some("reference"),
            tags: &tags,
            scope: None,
            split_level: first.split_level,
            sections: &first.sections,
            source_path: None,
        });

        let second = split_markdown(&rendered, "docs/auth.md");
        assert_eq!(second.title, first.title);
        assert_eq!(second.sections.len(), first.sections.len());
        for (a, b) in second.sections.iter().zip(first.sections.iter()) {
            assert_eq!(a.title, b.title);
            assert_eq!(a.content, b.content);
        }
    }

    #[test]
    fn round_trips_decision_context() {
        let md = "---\ntitle: Decisions\n---\n\n## Token Strategy\n\nWe use JWT.\n\n> **Rationale:** Stateless auth.\n\n> **Alternatives:**\n> - Sessions\n> - OAuth\n\n> **Consequences:** Need refresh tokens.";

        let first = split_markdown_with_level(md, "test.md", None);
        assert_eq!(
            first.sections[0].rationale.as_deref(),
            Some("Stateless auth.")
        );

        let tags: Vec<String> = Vec::new();
        let rendered = render_markdown(RenderOptions {
            title: &first.title,
            doc_type: None,
            tags: &tags,
            scope: None,
            split_level: first.split_level,
            sections: &first.sections,
            source_path: None,
        });

        let second = split_markdown(&rendered, "test.md");
        assert_eq!(second.sections[0].rationale, first.sections[0].rationale);
        assert_eq!(
            second.sections[0].alternatives,
            first.sections[0].alternatives
        );
        assert_eq!(
            second.sections[0].consequences,
            first.sections[0].consequences
        );
        assert_eq!(second.sections[0].content, first.sections[0].content);
    }
}
