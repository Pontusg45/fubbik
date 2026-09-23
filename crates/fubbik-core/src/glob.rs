//! Glob matching for `applies_to` patterns. Ports
//! `packages/api/src/context-for-file/glob-match.ts` (`normalizePath`,
//! `globMatch`) verbatim.
//!
//! Moved here from `fubbik-api::context::resolvers` (Task 5) ahead of
//! Task 7's `get_context_for_file`, which needs the identical logic from
//! the same Node source — `fubbik-core` is the right home because this is
//! pure logic with no database, HTTP or Ollama dependency, the same bar
//! `tokens`, `score` and `format` meet.
//!
//! Node builds `globMatch` by turning the pattern into a `RegExp`
//! (`.replace(/\./g, "\\.").replace(/\*\*/g, ...).replace(/\*/g,
//! ...).replace(/\?/g, ...)`, anchored `^...$`). No `regex` crate is a
//! workspace dependency, and pulling one in for a single small matcher
//! would be a heavier footprint than a direct backtracking implementation
//! of the same four token classes (literal, `*`, `**`, `?`) the TS regex
//! encodes. `tokenize` mirrors the TS `.replace()` chain's left-to-right,
//! non-overlapping scan — `***` becomes `Globstar` then `Star`, exactly as
//! three sequential `.replace(/\*\*/g, ...)` passes would consume it.

/// Ports `normalizePath` (`glob-match.ts:1-7`).
pub fn normalize_path(path: &str) -> String {
    let s = path.strip_prefix("./").unwrap_or(path);
    let s = s.strip_prefix('/').unwrap_or(s);
    let mut collapsed = String::with_capacity(s.len());
    let mut prev_slash = false;
    for c in s.chars() {
        if c == '/' {
            if prev_slash {
                continue;
            }
            prev_slash = true;
        } else {
            prev_slash = false;
        }
        collapsed.push(c);
    }
    collapsed
        .strip_suffix('/')
        .unwrap_or(&collapsed)
        .to_string()
}

#[derive(Debug, Clone, Copy)]
enum GlobToken {
    Lit(char),
    Star,
    Globstar,
    Question,
}

fn tokenize(pattern: &str) -> Vec<GlobToken> {
    let chars: Vec<char> = pattern.chars().collect();
    let mut tokens = Vec::with_capacity(chars.len());
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '*' if chars.get(i + 1) == Some(&'*') => {
                tokens.push(GlobToken::Globstar);
                i += 2;
            }
            '*' => {
                tokens.push(GlobToken::Star);
                i += 1;
            }
            '?' => {
                tokens.push(GlobToken::Question);
                i += 1;
            }
            c => {
                tokens.push(GlobToken::Lit(c));
                i += 1;
            }
        }
    }
    tokens
}

/// Backtracking match of `tokens` against `text`, both already normalized.
/// `Star`/`Question` never consume `/`, matching the TS regex's `[^/]*`/
/// `[^/]`; `Globstar` consumes anything, matching `.*`.
fn match_tokens(tokens: &[GlobToken], text: &[char]) -> bool {
    match tokens.first() {
        None => text.is_empty(),
        Some(GlobToken::Lit(c)) => {
            text.first() == Some(c) && match_tokens(&tokens[1..], &text[1..])
        }
        Some(GlobToken::Question) => match text.first() {
            Some(&t0) if t0 != '/' => match_tokens(&tokens[1..], &text[1..]),
            _ => false,
        },
        Some(GlobToken::Star) => {
            let mut end = 0;
            loop {
                if match_tokens(&tokens[1..], &text[end..]) {
                    return true;
                }
                if end >= text.len() || text[end] == '/' {
                    return false;
                }
                end += 1;
            }
        }
        Some(GlobToken::Globstar) => {
            let mut end = 0;
            loop {
                if match_tokens(&tokens[1..], &text[end..]) {
                    return true;
                }
                if end >= text.len() {
                    return false;
                }
                end += 1;
            }
        }
    }
}

/// Ports `globMatch` (`glob-match.ts:9-19`).
pub fn glob_match(pattern: &str, path: &str) -> bool {
    let normalized_pattern = normalize_path(pattern);
    let normalized_path = normalize_path(path);
    let tokens = tokenize(&normalized_pattern);
    let text: Vec<char> = normalized_path.chars().collect();
    match_tokens(&tokens, &text)
}

#[cfg(test)]
mod tests {
    use super::{glob_match, normalize_path};

    // --- normalize_path — mirrors glob-match.test.ts's `normalizePath` describe block ---

    #[test]
    fn strips_leading_dot_slash() {
        // Given the inline inputs and test fixtures.
        // When the operation is evaluated by the assertion.
        // Then
        assert_eq!(
            normalize_path("./src/auth/service.ts"),
            "src/auth/service.ts"
        );
    }

    #[test]
    fn strips_leading_slash() {
        // Given the inline inputs and test fixtures.
        // When the operation is evaluated by the assertion.
        // Then
        assert_eq!(
            normalize_path("/src/auth/service.ts"),
            "src/auth/service.ts"
        );
    }

    #[test]
    fn collapses_consecutive_slashes() {
        // Given the inline inputs and test fixtures.
        // When the operation is evaluated by the assertion.
        // Then
        assert_eq!(
            normalize_path("src//auth///service.ts"),
            "src/auth/service.ts"
        );
    }

    #[test]
    fn strips_trailing_slash() {
        // Given the inline inputs and test fixtures.
        // When the operation is evaluated by the assertion.
        // Then
        assert_eq!(normalize_path("src/auth/"), "src/auth");
    }

    #[test]
    fn handles_combined_edge_cases() {
        // Given the inline inputs and test fixtures.
        // When the operation is evaluated by the assertion.
        // Then
        assert_eq!(
            normalize_path("./src//auth/./service.ts"),
            "src/auth/./service.ts"
        );
    }

    #[test]
    fn returns_empty_string_unchanged() {
        // Given the inline inputs and test fixtures.
        // When the operation is evaluated by the assertion.
        // Then
        assert_eq!(normalize_path(""), "");
    }

    #[test]
    fn handles_already_clean_paths() {
        // Given the inline inputs and test fixtures.
        // When the operation is evaluated by the assertion.
        // Then
        assert_eq!(normalize_path("src/auth/service.ts"), "src/auth/service.ts");
    }

    // --- glob_match with normalization — mirrors glob-match.test.ts's second describe block ---

    #[test]
    fn matches_dot_slash_prefixed_path_against_globstar_pattern() {
        // Given the inline inputs and test fixtures.
        // When the operation is evaluated by the assertion.
        // Then
        assert!(glob_match("src/**/*.ts", "./src/auth/service.ts"));
    }

    #[test]
    fn matches_leading_slash_path_against_globstar_pattern() {
        // Given the inline inputs and test fixtures.
        // When the operation is evaluated by the assertion.
        // Then
        assert!(glob_match("src/**/*.ts", "/src/auth/service.ts"));
    }

    #[test]
    fn normalizes_the_pattern_too() {
        // Given the inline inputs and test fixtures.
        // When the operation is evaluated by the assertion.
        // Then
        assert!(glob_match("./src/**/*.ts", "src/auth/service.ts"));
    }

    // --- glob_match — carried over from the original Task 5 unit tests (resolvers.rs) ---

    #[test]
    fn exact_literal_path_matches_itself() {
        // Given the inline inputs and test fixtures.
        // When the operation is evaluated by the assertion.
        // Then
        assert!(glob_match("src/foo.rs", "src/foo.rs"));
        assert!(!glob_match("src/foo.rs", "src/bar.rs"));
    }

    #[test]
    fn single_star_does_not_cross_a_slash() {
        // Given the inline inputs and test fixtures.
        // When the operation is evaluated by the assertion.
        // Then
        assert!(glob_match("src/*.rs", "src/foo.rs"));
        assert!(!glob_match("src/*.rs", "src/nested/foo.rs"));
    }

    #[test]
    fn double_star_crosses_slashes() {
        // Given the inline inputs and test fixtures.
        // When the operation is evaluated by the assertion.
        // Then
        assert!(glob_match("src/**/*.rs", "src/a/b/foo.rs"));
    }

    /// The surrounding literal `/`s in `src/**/*.rs` are NOT consumed by
    /// `**` itself — Node's regex is `src/.*\/[^/]*\.rs` (verified against
    /// the live `packages/api/src/context-for-file/glob-match.ts` logic),
    /// which needs a second, separate `/` somewhere after the globstar. A
    /// zero-depth path with no intermediate directory has nowhere for that
    /// second `/` to come from, so it does NOT match — caught here after an
    /// earlier version of this test (written before this file existed,
    /// under `context::resolvers`) asserted the opposite and had never
    /// actually been run as part of Task 5's `--test context` suite, since
    /// unit tests live in a different `cargo test` target.
    #[test]
    fn double_star_requires_a_real_directory_segment_either_side() {
        // Given the inline inputs and test fixtures.
        // When the operation is evaluated by the assertion.
        // Then
        assert!(!glob_match("src/**/*.rs", "src/foo.rs"));
    }

    #[test]
    fn leading_and_trailing_slashes_and_dot_slash_are_normalized_away() {
        // Given the inline inputs and test fixtures.
        // When the operation is evaluated by the assertion.
        // Then
        assert!(glob_match("./src/foo.rs", "/src/foo.rs/"));
    }

    #[test]
    fn dot_is_literal_not_any_char() {
        // Given the inline inputs and test fixtures.
        // When the operation is evaluated by the assertion.
        // Then
        assert!(!glob_match("src/foo.rs", "src/fooXrs"));
    }
}
