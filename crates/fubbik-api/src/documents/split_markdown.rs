//! Port of `packages/api/src/documents/split-markdown.ts`. No `regex`
//! dependency — see `frontmatter.rs`'s module doc comment for why; every
//! pattern below is hand-matched with plain string/byte operations,
//! chosen to reproduce each regex's exact matching semantics (documented
//! per-helper below), not just its common-case behaviour.

use super::frontmatter::{extract_frontmatter, strip_md_suffix_ci, tags_from_path};

#[derive(Debug, Clone, PartialEq)]
pub struct MarkdownSection {
    pub title: String,
    pub content: String,
    pub order: i32,
    pub rationale: Option<String>,
    pub alternatives: Option<Vec<String>>,
    pub consequences: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SplitResult {
    pub title: String,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub sections: Vec<MarkdownSection>,
    pub split_level: i32,
}

/// Equivalent to Node's `splitMarkdown(raw, filePath)` — no explicit
/// `splitLevel` override, i.e. auto-detect.
pub fn split_markdown(raw: &str, file_path: &str) -> SplitResult {
    split_markdown_with_level(raw, file_path, None)
}

/// `split_level: None` is Node's `undefined`/`"auto"` (auto-detect via
/// [`detect_split_level`]); `Some(n)` is an explicit override. Node's
/// union type also allows the literal string `"auto"` — never passed by
/// any real caller in this domain (only `split-markdown.test.ts` exercises
/// the numeric-override branch) — so it isn't modelled here.
pub fn split_markdown_with_level(
    raw: &str,
    file_path: &str,
    split_level: Option<u8>,
) -> SplitResult {
    let (frontmatter, body) = extract_frontmatter(raw);
    let mut title = frontmatter.title;
    let mut content = body;

    if let Some((h1_title, rest)) = strip_first_h1(&content) {
        if title.is_none() {
            title = Some(h1_title);
        }
        content = rest;
    }

    let title = title.unwrap_or_else(|| derive_title_from_path(file_path));

    let tags = dedup_preserve_order(
        frontmatter
            .tags
            .into_iter()
            .chain(tags_from_path(file_path)),
    );
    let description = frontmatter.description;

    let resolved_level = match split_level {
        Some(l) => l as usize,
        None => detect_split_level(&content),
    };

    let headings = find_headings(&content, resolved_level);

    let mut sections = Vec::new();
    let mut order = 0i32;

    if headings.is_empty() {
        let trimmed = content.trim();
        if !trimmed.is_empty() {
            sections.push(intro_section(&title, trimmed, 0));
        }
        return SplitResult {
            title,
            description,
            tags,
            sections,
            split_level: resolved_level as i32,
        };
    }

    let preamble = content[..headings[0].line_start].trim();
    if !preamble.is_empty() && !contains_heading_line_2_to_6(preamble) {
        sections.push(intro_section(&title, preamble, order));
        order += 1;
    }

    for (i, heading) in headings.iter().enumerate() {
        let section_end = headings
            .get(i + 1)
            .map(|h| h.line_start)
            .unwrap_or(content.len());
        let raw_section = content[heading.content_start..section_end].trim();
        let (clean_content, ctx) = extract_decision_context(raw_section);
        sections.push(MarkdownSection {
            title: heading.title.clone(),
            content: clean_content,
            order,
            rationale: ctx.rationale,
            alternatives: ctx.alternatives,
            consequences: ctx.consequences,
        });
        order += 1;
    }

    SplitResult {
        title,
        description,
        tags,
        sections,
        split_level: resolved_level as i32,
    }
}

fn intro_section(title: &str, content: &str, order: i32) -> MarkdownSection {
    MarkdownSection {
        title: format!("{title} \u{2014} Introduction"),
        content: content.to_string(),
        order,
        rationale: None,
        alternatives: None,
        consequences: None,
    }
}

fn dedup_preserve_order(items: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for item in items {
        if seen.insert(item.clone()) {
            out.push(item);
        }
    }
    out
}

/// Returns `Some(n)` when `line` starts with exactly `n` `#` characters
/// immediately followed by a space, for any `n >= 1`. Mirrors the
/// greedy-then-backtrack behaviour of a fixed-count-range regex like
/// `#{2,6} `: it only matches when the *actual* leading-hash run length
/// equals the checked count — never when more hashes follow (the char
/// right after the checked count would then still be `#`, not a space).
fn heading_hash_count(line: &str) -> Option<usize> {
    let bytes = line.as_bytes();
    let n = bytes.iter().take_while(|b| **b == b'#').count();
    if n == 0 {
        return None;
    }
    if bytes.get(n) == Some(&b' ') {
        Some(n)
    } else {
        None
    }
}

/// `content.match(/^#{2,6} /m)` -> `match[0].trimEnd().length`, defaulting
/// to `2` when nothing matches.
fn detect_split_level(content: &str) -> usize {
    for line in content.split('\n') {
        if let Some(n) = heading_hash_count(line)
            && (2..=6).contains(&n)
        {
            return n;
        }
    }
    2
}

/// `/^#{2,6} /m.test(text)` — used only for the preamble-has-a-heading
/// guard, independent of the document's actual `resolvedLevel`.
fn contains_heading_line_2_to_6(text: &str) -> bool {
    text.split('\n')
        .any(|line| matches!(heading_hash_count(line), Some(n) if (2..=6).contains(&n)))
}

/// `/^#\s+(.+)$/m` (find) + `.replace(/^#\s+.+\n?/m, "")` (remove) — the
/// single leading H1, wherever the first one appears (not necessarily at
/// byte 0). Requires *exactly* one `#` (a second `#` right after would be
/// non-whitespace, so `\s+` wouldn't match there).
fn strip_first_h1(content: &str) -> Option<(String, String)> {
    let mut offset = 0usize;
    for line in content.split('\n') {
        if let Some(rest) = line.strip_prefix('#')
            && rest.starts_with(|c: char| c.is_whitespace())
        {
            let title = rest.trim().to_string();
            let line_len = line.len();
            let mut new_content = String::with_capacity(content.len() - line_len);
            new_content.push_str(&content[..offset]);
            let after = offset + line_len;
            let after = if content[after..].starts_with('\n') {
                after + 1
            } else {
                after
            };
            new_content.push_str(&content[after..]);
            return Some((title, new_content.trim().to_string()));
        }
        offset += line.len() + 1;
    }
    None
}

fn derive_title_from_path(path: &str) -> String {
    let filename = path.rsplit('/').next().unwrap_or(path);
    let stem = strip_md_suffix_ci(filename);
    stem.chars()
        .map(|c| if c == '-' || c == '_' { ' ' } else { c })
        .collect()
}

struct HeadingMatch {
    title: String,
    /// Byte offset (into `content`) of the heading line's first `#`.
    line_start: usize,
    /// Byte offset right after the heading line's trailing newline (or
    /// `content.len()` if the heading is the last line with no trailing
    /// newline).
    content_start: usize,
}

/// `new RegExp(\`^${prefix} (.+)$\`, "gm")` for a fixed `prefix` (`"#".repeat(level)`)
/// — every line whose leading-hash run is exactly `level` long, in order.
fn find_headings(content: &str, level: usize) -> Vec<HeadingMatch> {
    let mut heads = Vec::new();
    let mut offset = 0usize;
    for line in content.split('\n') {
        if heading_hash_count(line) == Some(level) {
            let title = line[level + 1..].trim().to_string();
            let content_start = (offset + line.len() + 1).min(content.len());
            heads.push(HeadingMatch {
                title,
                line_start: offset,
                content_start,
            });
        }
        offset += line.len() + 1;
    }
    heads
}

#[derive(Default)]
struct DecisionContext {
    rationale: Option<String>,
    alternatives: Option<Vec<String>>,
    consequences: Option<String>,
}

/// Port of `extractDecisionContext`
/// (`packages/api/src/documents/split-markdown.ts:32-77`): scans
/// backwards from the end of `content` for a trailing run of blank/`>`-
/// prefixed lines; if that trailing block contains any of the three
/// `> **X:**` markers, pulls rationale/alternatives/consequences out of it
/// and returns the content with that trailing block removed (right-trimmed
/// only — matching Node's `.trimEnd()`, not a full `.trim()`).
fn extract_decision_context(content: &str) -> (String, DecisionContext) {
    let lines: Vec<&str> = content.split('\n').collect();
    let mut trailing_start = lines.len();
    for i in (0..lines.len()).rev() {
        let line = lines[i].trim();
        if line.is_empty() || line.starts_with('>') {
            trailing_start = i;
        } else {
            break;
        }
    }
    let trailing_block = lines[trailing_start..].join("\n");

    let has_decision_context = trailing_block.contains("> **Rationale:**")
        || trailing_block.contains("> **Alternatives:**")
        || trailing_block.contains("> **Consequences:**");

    if !has_decision_context {
        return (content.to_string(), DecisionContext::default());
    }

    let ctx = DecisionContext {
        rationale: match_scalar_directive(&trailing_block, "> **Rationale:**"),
        alternatives: extract_alternatives(&trailing_block),
        consequences: match_scalar_directive(&trailing_block, "> **Consequences:**"),
    };

    let clean_content = lines[..trailing_start].join("\n").trim_end().to_string();
    (clean_content, ctx)
}

/// `/> \*\*{Marker}:\*\*\s*(.*)/` — finds the marker anywhere, skips
/// whitespace (including newlines, matching JS `\s`), then captures the
/// rest of the resulting line. Always returns `Some` (possibly an empty
/// string) once the marker text is found, matching `.*`'s ability to
/// match zero characters — a marker with nothing after it still "matches",
/// same as Node's `if (match) context.x = match[1].trim()`.
fn match_scalar_directive(block: &str, marker: &str) -> Option<String> {
    let idx = block.find(marker)?;
    let after = block[idx + marker.len()..].trim_start();
    let line_end = after.find('\n').unwrap_or(after.len());
    Some(after[..line_end].trim().to_string())
}

/// `/> \*\*Alternatives:\*\*\s*([\s\S]*?)(?=\n> \*\*(?:Rationale|Consequences):\*\*|\n[^>\n]|$)/`
/// followed by `altBlock.match(/^>\s*-\s+(.+)$/gm)`. Implemented as: find
/// the line carrying the marker, then scan forward line-by-line, stopping
/// (exclusive) at the first line that either starts with a Rationale/
/// Consequences marker or is non-blank and doesn't start with `>` — the
/// same three stop conditions as the lookahead, evaluated per-line instead
/// of per-character since all three are anchored to line starts. Returns
/// `None` (Node: field left unset) when no `> - item` lines are found in
/// that span, even if the `Alternatives:` marker itself was present.
fn extract_alternatives(block: &str) -> Option<Vec<String>> {
    let marker = "> **Alternatives:**";
    let lines: Vec<&str> = block.split('\n').collect();
    let marker_line_idx = lines.iter().position(|l| l.contains(marker))?;

    let mut end_idx = lines.len();
    for (j, line) in lines.iter().enumerate().skip(marker_line_idx + 1) {
        if line.starts_with("> **Rationale:**") || line.starts_with("> **Consequences:**") {
            end_idx = j;
            break;
        }
        if !line.is_empty() && !line.starts_with('>') {
            end_idx = j;
            break;
        }
    }

    let mut items = Vec::new();
    for line in &lines[marker_line_idx..end_idx] {
        if let Some(item) = alternative_item(line) {
            items.push(item);
        }
    }

    if items.is_empty() { None } else { Some(items) }
}

/// `^>\s*-\s+(.+)$` for a single line.
fn alternative_item(line: &str) -> Option<String> {
    let rest = line.strip_prefix('>')?.trim_start();
    let rest = rest.strip_prefix('-')?;
    if !rest.starts_with(|c: char| c.is_whitespace()) {
        return None;
    }
    let content = rest.trim();
    if content.is_empty() {
        None
    } else {
        Some(content.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_on_h2_headings() {
        let md = "# My Document\n\nIntro paragraph.\n\n## First Section\n\nFirst content.\n\n## Second Section\n\nSecond content.\n";
        let result = split_markdown(md, "docs/test.md");
        assert_eq!(result.title, "My Document");
        assert_eq!(result.sections.len(), 3);
        assert_eq!(
            result.sections[0].title,
            "My Document \u{2014} Introduction"
        );
        assert_eq!(result.sections[0].content, "Intro paragraph.");
        assert_eq!(result.sections[0].order, 0);
        assert_eq!(result.sections[1].title, "First Section");
        assert_eq!(result.sections[1].content, "First content.");
        assert_eq!(result.sections[1].order, 1);
        assert_eq!(result.sections[2].title, "Second Section");
        assert_eq!(result.sections[2].content, "Second content.");
        assert_eq!(result.sections[2].order, 2);
    }

    #[test]
    fn skips_empty_preamble() {
        let md = "# Title\n\n## Only Section\n\nContent here.\n";
        let result = split_markdown(md, "test.md");
        assert_eq!(result.sections.len(), 1);
        assert_eq!(result.sections[0].title, "Only Section");
        assert_eq!(result.sections[0].order, 0);
    }

    #[test]
    fn falls_back_to_filename_for_title() {
        let md = "## Section One\n\nContent.\n";
        let result = split_markdown(md, "docs/my-cool-guide.md");
        assert_eq!(result.title, "my cool guide");
    }

    #[test]
    fn preserves_h3_plus_subheadings_within_sections() {
        let md = "# Doc\n\n## Main\n\n### Sub\n\nDetails.\n\n#### Deep\n\nMore.\n";
        let result = split_markdown(md, "test.md");
        assert_eq!(result.sections.len(), 1);
        assert!(result.sections[0].content.contains("### Sub"));
        assert!(result.sections[0].content.contains("#### Deep"));
    }

    #[test]
    fn extracts_frontmatter_tags_and_description() {
        let md = "---\ntags:\n  - backend\n  - auth\ndescription: A guide to auth\n---\n\n# Auth Guide\n\n## Setup\n\nSteps here.\n";
        let result = split_markdown(md, "docs/auth.md");
        assert_eq!(result.title, "Auth Guide");
        assert_eq!(result.tags, vec!["backend", "auth", "docs"]);
        assert_eq!(result.description.as_deref(), Some("A guide to auth"));
    }

    #[test]
    fn treats_whole_file_as_single_section_when_no_h2s() {
        let md = "# Simple Note\n\nJust some content with no H2 headings.\n";
        let result = split_markdown(md, "note.md");
        assert_eq!(result.sections.len(), 1);
        assert_eq!(
            result.sections[0].title,
            "Simple Note \u{2014} Introduction"
        );
        assert_eq!(
            result.sections[0].content,
            "Just some content with no H2 headings."
        );
    }

    #[test]
    fn auto_detects_h3_as_split_level_when_no_h2s_exist() {
        let md = "# Title\n\n### First\n\nContent one.\n\n### Second\n\nContent two.\n";
        let result = split_markdown(md, "test.md");
        assert_eq!(result.split_level, 3);
        assert_eq!(result.sections.len(), 2);
        assert_eq!(result.sections[0].title, "First");
        assert_eq!(result.sections[1].title, "Second");
    }

    #[test]
    fn uses_explicit_split_level_override() {
        let md = "# Title\n\n## H2 Section\n\nContent.\n\n### H3 Section\n\nMore.\n";
        let result = split_markdown_with_level(md, "test.md", Some(3));
        assert_eq!(result.split_level, 3);
        assert_eq!(result.sections.len(), 1);
        assert_eq!(result.sections[0].title, "H3 Section");
    }

    #[test]
    fn returns_split_level_2_for_existing_h2_documents() {
        let md = "# My Document\n\nIntro.\n\n## First Section\n\nFirst content.\n";
        let result = split_markdown(md, "test.md");
        assert_eq!(result.split_level, 2);
    }

    #[test]
    fn defaults_split_level_to_2_when_no_headings_found() {
        let md = "# Title\n\nJust content with no sub-headings.\n";
        let result = split_markdown(md, "test.md");
        assert_eq!(result.split_level, 2);
        assert_eq!(result.sections.len(), 1);
        assert_eq!(result.sections[0].title, "Title \u{2014} Introduction");
    }

    #[test]
    fn extracts_decision_context_from_trailing_blockquotes() {
        let md = "# Doc\n\n## Auth\n\nWe use JWT for authentication.\n\n> **Rationale:** Stateless, no server-side sessions needed.\n\n> **Alternatives:**\n> - Session cookies\n> - OAuth tokens\n\n> **Consequences:** Requires token refresh logic.";
        let result = split_markdown(md, "test.md");
        assert_eq!(result.sections.len(), 1);
        assert_eq!(result.sections[0].content, "We use JWT for authentication.");
        assert_eq!(
            result.sections[0].rationale.as_deref(),
            Some("Stateless, no server-side sessions needed.")
        );
        assert_eq!(
            result.sections[0].alternatives,
            Some(vec![
                "Session cookies".to_string(),
                "OAuth tokens".to_string()
            ])
        );
        assert_eq!(
            result.sections[0].consequences.as_deref(),
            Some("Requires token refresh logic.")
        );
    }

    #[test]
    fn does_not_extract_blockquotes_that_are_not_decision_context() {
        let md = "# Doc\n\n## Notes\n\n> This is a regular blockquote in the middle.\n\nMore content after the blockquote.";
        let result = split_markdown(md, "test.md");
        assert!(
            result.sections[0]
                .content
                .contains("> This is a regular blockquote")
        );
        assert!(
            result.sections[0]
                .content
                .contains("More content after the blockquote.")
        );
        assert_eq!(result.sections[0].rationale, None);
    }

    #[test]
    fn handles_partial_decision_context_only_rationale() {
        let md = "# Doc\n\n## Design\n\nWe chose X.\n\n> **Rationale:** Because Y.";
        let result = split_markdown(md, "test.md");
        assert_eq!(result.sections[0].content, "We chose X.");
        assert_eq!(result.sections[0].rationale.as_deref(), Some("Because Y."));
        assert_eq!(result.sections[0].alternatives, None);
        assert_eq!(result.sections[0].consequences, None);
    }

    #[test]
    fn handles_markdown_with_only_frontmatter_and_content() {
        let md = "---\ntitle: Quick Reference\ntags:\n  - reference\ndescription: A quick ref card\n---\n\nJust a simple reference document with no sections.\n";
        let result = split_markdown(md, "ref.md");
        assert_eq!(result.title, "Quick Reference");
        assert_eq!(result.description.as_deref(), Some("A quick ref card"));
        assert_eq!(result.sections.len(), 1);
        assert_eq!(
            result.sections[0].content,
            "Just a simple reference document with no sections."
        );
    }

    #[test]
    fn handles_empty_file_gracefully() {
        let result = split_markdown("", "empty.md");
        assert_eq!(result.title, "empty");
        assert_eq!(result.sections.len(), 0);
    }
}
