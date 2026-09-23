//! Direct port of `packages/api/src/requirements/cross-ref.ts` —
//! best-effort scan of a requirement's step text for file-path-shaped
//! tokens, flagging ones that don't match any of the caller's chunk file
//! references. Always degrades to an empty warning list on any failure
//! (unreadable step text, DB hiccup); never blocks create/update.

use fubbik_db::repo::requirement::RequirementStep;
use sqlx::PgPool;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum CrossRefWarningType {
    FileNotFound,
    ChunkNotFound,
}

/// Matches Node's `CrossRefWarning` (`packages/api/src/requirements/
/// cross-ref.ts:5-9`). Node's `type` union also names `"chunk_not_found"`,
/// but nothing in `crossReferenceSteps`'s body ever produces it (the only
/// `warnings.push` call in the function hard-codes `"file_not_found"`) — an
/// unreachable variant kept here only because it is part of the type Node
/// declares, matching this port's usual "reproduce the declared shape,
/// even the unreachable branch" convention for tagged unions.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, utoipa::ToSchema)]
pub struct CrossRefWarning {
    pub step: i32,
    #[serde(rename = "type")]
    pub warning_type: CrossRefWarningType,
    pub reference: String,
}

/// Word/path characters allowed inside a candidate file path token, per
/// Node's `[\w./-]` character class (`\w` = `[A-Za-z0-9_]`).
fn is_path_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '/' || c == '-'
}

/// Extracts file-path-shaped tokens from `text`. Node's regex
/// (`/(?:^|\s)([\w./-]+\.\w{1,10})(?:\s|$|[,;:)])/g`) is reproduced here as
/// a hand-rolled whitespace-delimited scan rather than pulling in a
/// `regex` dependency (same rationale as `export::interpolate`): split on
/// whitespace, strip at most one trailing punctuation character from the
/// set `,;:)` (the regex's trailing boundary class), then check the
/// remainder is all path characters and ends in `.` + 1-10 word
/// characters. Requires at least one `/`, matching Node's separate
/// `path.includes("/")` filter. Deduplicated, matching Node's
/// `[...new Set(paths)]`.
fn extract_file_paths(text: &str) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();

    for word in text.split_whitespace() {
        let mut candidate = word;
        if let Some(last) = candidate.chars().last()
            && matches!(last, ',' | ';' | ':' | ')')
        {
            candidate = &candidate[..candidate.len() - last.len_utf8()];
        }
        if candidate.is_empty() || !candidate.chars().all(is_path_char) {
            continue;
        }
        let Some(dot) = candidate.rfind('.') else {
            continue;
        };
        let ext = &candidate[dot + 1..];
        let ext_len = ext.chars().count();
        if !(1..=10).contains(&ext_len)
            || !ext.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            continue;
        }
        if !candidate.contains('/') {
            continue;
        }
        if seen.insert(candidate.to_string()) {
            out.push(candidate.to_string());
        }
    }

    out
}

/// Matches Node's `crossReferenceSteps`
/// (`packages/api/src/requirements/cross-ref.ts:27-52`): for every
/// file-path-shaped token in every step's text, flag it if no chunk owned
/// by `user_id` references that exact path. Any DB error degrades to an
/// empty warning list, same as Node's `catchAll(() => Effect.succeed([]))`.
pub async fn cross_reference_steps(
    pool: &PgPool,
    user_id: &str,
    steps: &[RequirementStep],
) -> Vec<CrossRefWarning> {
    let mut warnings = Vec::new();

    for (i, step) in steps.iter().enumerate() {
        for path in extract_file_paths(&step.text) {
            let exists = fubbik_db::repo::chunk_meta::file_ref_path_exists(pool, user_id, &path)
                .await
                .unwrap_or(true); // degrade to "no warning" on error, matching Node
            if !exists {
                warnings.push(CrossRefWarning {
                    step: i as i32,
                    warning_type: CrossRefWarningType::FileNotFound,
                    reference: path,
                });
            }
        }
    }

    warnings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_slash_containing_paths_with_extensions() {
        // Given the inline inputs and test fixtures.
        // When
        let paths = extract_file_paths("see src/lib.rs and also docs/readme.md for details");
        // Then
        assert_eq!(
            paths,
            vec!["src/lib.rs".to_string(), "docs/readme.md".to_string()]
        );
    }

    #[test]
    fn ignores_tokens_without_a_slash() {
        // Given the inline inputs and test fixtures.
        // When the operation is evaluated by the assertion.
        // Then
        assert!(extract_file_paths("update Cargo.toml please").is_empty());
    }

    #[test]
    fn strips_trailing_punctuation() {
        // Given the inline inputs and test fixtures.
        // When
        let paths = extract_file_paths("check src/main.rs, then commit.");
        // Then
        assert_eq!(paths, vec!["src/main.rs".to_string()]);
    }

    #[test]
    fn dedupes_repeated_paths() {
        // Given the inline inputs and test fixtures.
        // When
        let paths = extract_file_paths("src/lib.rs then src/lib.rs again");
        // Then
        assert_eq!(paths, vec!["src/lib.rs".to_string()]);
    }
}
