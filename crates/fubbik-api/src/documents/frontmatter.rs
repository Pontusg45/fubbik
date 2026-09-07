//! Hand-rolled port of the pieces of Node's `extractFrontmatter`/
//! `tagsFromPath` (`packages/api/src/chunks/parse-docs.ts:60-116`) that
//! `split_markdown`/`render_markdown` actually consume.
//!
//! The wider template-aware parser now lives in
//! `documents::template_import`; this smaller parser remains focused on
//! what `split_markdown` and `render_markdown` read: `frontmatter.title`,
//! `frontmatter.tags`, and `frontmatter.description` — never the
//! nested-object `scope` frontmatter key parse-docs.ts also supports (only
//! `parseDocFile` reads that). This port therefore does not implement that
//! nested-object branch of Node's mini-YAML parser; it is dead code for
//! every reachable caller in this domain.
//!
//! No `regex` dependency: matching this codebase's established convention
//! (no text-parsing domain ported so far — `search::parser`, staleness
//! scanning, etc. — pulls in the `regex` crate), every pattern below is
//! hand-matched with plain string/byte operations instead.

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Frontmatter {
    pub title: Option<String>,
    pub description: Option<String>,
    pub tags: Vec<String>,
}

enum FmValue {
    Scalar(String),
    List(Vec<String>),
}

/// Splits a leading `---\n...\n---\n?` block from `raw`, returning the
/// parsed frontmatter and the remaining body — mirrors Node's
/// `extractFrontmatter` exactly, including the asymmetry in how `body` is
/// produced: **trimmed** when a frontmatter block was found, but the
/// **untouched original `raw`** when none was found
/// (`packages/api/src/chunks/parse-docs.ts:61-64` vs. `:86` — Node returns
/// `{ frontmatter: {}, body: raw }` on a regex miss, never trimming `raw`).
pub fn extract_frontmatter(raw: &str) -> (Frontmatter, String) {
    match split_frontmatter_block(raw) {
        Some((yaml, body)) => {
            let mut fm = Frontmatter::default();
            for (key, value) in parse_yaml_lines(yaml) {
                match (key.as_str(), value) {
                    ("title", FmValue::Scalar(s)) => fm.title = Some(s),
                    ("description", FmValue::Scalar(s)) => fm.description = Some(s),
                    ("tags", FmValue::List(items)) => fm.tags = items,
                    // Any other key, or a shape mismatch (e.g. a scalar
                    // `tags: foo` line, which Node would store as a plain
                    // string that `Array.isArray(frontmatter.tags)` then
                    // rejects back down to `[]`), is simply not one of the
                    // three fields this domain reads — dropped.
                    _ => {}
                }
            }
            (fm, body.trim().to_string())
        }
        None => (Frontmatter::default(), raw.to_string()),
    }
}

/// `/^---\n([\s\S]*?)\n---\n?([\s\S]*)$/` with no flags — anchored to the
/// *whole string* (not per-line), and the lazy group stops at the very
/// first `"\n---"` found, with no requirement that the closing fence be
/// alone on its line (the regex's `\n?` after `---` is optional, so
/// whatever immediately follows just becomes the start of `body`).
fn split_frontmatter_block(raw: &str) -> Option<(&str, &str)> {
    let after_open = raw.strip_prefix("---\n")?;
    let idx = after_open.find("\n---")?;
    let yaml = &after_open[..idx];
    let after_fence = &after_open[idx + 4..];
    let body = after_fence.strip_prefix('\n').unwrap_or(after_fence);
    Some((yaml, body))
}

/// Line-by-line state machine mirroring Node's `extractFrontmatter` loop
/// exactly (`packages/api/src/chunks/parse-docs.ts:75-108`): a `key:`
/// line with no value starts an array accumulation; subsequent `  - item`
/// lines feed it; any other line flushes the pending array under the
/// last-seen key. Last write wins if a key appears twice (matches plain
/// object-key assignment in JS).
fn parse_yaml_lines(yaml: &str) -> Vec<(String, FmValue)> {
    let mut result: Vec<(String, FmValue)> = Vec::new();
    let mut current_key: Option<String> = None;
    let mut current_array: Option<Vec<String>> = None;

    for line in yaml.split('\n') {
        if let Some(item) = array_item(line)
            && current_key.is_some()
        {
            current_array
                .get_or_insert_with(Vec::new)
                .push(item.trim().to_string());
            continue;
        }

        if let Some(key) = &current_key
            && let Some(arr) = current_array.take()
        {
            upsert(&mut result, key.clone(), FmValue::List(arr));
        }

        if let Some((key, value)) = kv_match(line) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                upsert(
                    &mut result,
                    key.clone(),
                    FmValue::Scalar(trimmed.to_string()),
                );
                current_key = None;
            } else {
                current_key = Some(key);
            }
        }
    }

    if let Some(key) = &current_key
        && let Some(arr) = current_array.take()
    {
        upsert(&mut result, key.clone(), FmValue::List(arr));
    }

    result
}

fn upsert(result: &mut Vec<(String, FmValue)>, key: String, value: FmValue) {
    if let Some(entry) = result.iter_mut().find(|(k, _)| *k == key) {
        entry.1 = value;
    } else {
        result.push((key, value));
    }
}

/// `^\s+-\s+(.+)$` — an indented list item line.
fn array_item(line: &str) -> Option<&str> {
    let rest = line.strip_prefix(|c: char| c.is_whitespace())?.trim_start();
    let rest = rest.strip_prefix('-')?;
    let rest = rest.strip_prefix(|c: char| c.is_whitespace())?.trim_start();
    if rest.is_empty() { None } else { Some(rest) }
}

/// `^(\w+):\s*(.*)$` — a top-level `key: value` (or bare `key:`) line.
/// `\w` is `[A-Za-z0-9_]` under JS's default (non-Unicode) regex flags.
fn kv_match(line: &str) -> Option<(String, &str)> {
    let mut end = 0usize;
    for (i, c) in line.char_indices() {
        if c.is_ascii_alphanumeric() || c == '_' {
            end = i + c.len_utf8();
        } else {
            break;
        }
    }
    if end == 0 {
        return None;
    }
    let rest = line[end..].strip_prefix(':')?;
    Some((line[..end].to_string(), rest))
}

/// Port of `tagsFromPath` (`packages/api/src/chunks/parse-docs.ts:52-61`):
/// every non-empty path segment except the filename, plus the filename's
/// own stem (`.md` stripped, `-`/`_` -> space) unless that stem is empty,
/// `"index"`, or `"readme"` (case-insensitively).
pub fn tags_from_path(path: &str) -> Vec<String> {
    let mut parts: Vec<&str> = path.split('/').collect();
    let filename = parts.pop();
    let mut folders: Vec<String> = parts
        .into_iter()
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();

    if let Some(filename) = filename {
        let stem = strip_md_suffix_ci(filename);
        let stem: String = stem
            .chars()
            .map(|c| if c == '-' || c == '_' { ' ' } else { c })
            .collect();
        let stem = stem.trim().to_string();
        let lower = stem.to_lowercase();
        if !stem.is_empty() && lower != "index" && lower != "readme" {
            folders.push(stem);
        }
    }

    folders
}

/// `.replace(/\.md$/i, "")` — case-insensitive `.md` suffix strip.
pub(super) fn strip_md_suffix_ci(name: &str) -> &str {
    if name.len() >= 3 && name[name.len() - 3..].eq_ignore_ascii_case(".md") {
        &name[..name.len() - 3]
    } else {
        name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_scalar_and_list_fields() {
        let (fm, body) = extract_frontmatter(
            "---\ntitle: Quick Reference\ntags:\n  - reference\ndescription: A quick ref card\n---\n\nBody here.\n",
        );
        assert_eq!(fm.title.as_deref(), Some("Quick Reference"));
        assert_eq!(fm.description.as_deref(), Some("A quick ref card"));
        assert_eq!(fm.tags, vec!["reference".to_string()]);
        assert_eq!(body, "Body here.");
    }

    #[test]
    fn no_frontmatter_block_returns_untouched_raw_as_body() {
        let (fm, body) = extract_frontmatter("# Just a heading\n\nSome text.\n");
        assert_eq!(fm, Frontmatter::default());
        assert_eq!(body, "# Just a heading\n\nSome text.\n");
    }

    #[test]
    fn tags_from_path_drops_index_and_readme_stems() {
        assert_eq!(tags_from_path("docs/guide/index.md"), vec!["docs", "guide"]);
        assert_eq!(tags_from_path("docs/readme.md"), vec!["docs"]);
        assert_eq!(
            tags_from_path("docs/my-cool-guide.md"),
            vec!["docs", "my cool guide"]
        );
    }
}
