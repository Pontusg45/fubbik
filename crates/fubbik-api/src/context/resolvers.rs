//! Ports `packages/api/src/context/resolvers.ts`'s three input resolvers
//! (`resolveForPlan`, `resolveForConcept`, `resolveForFiles`). Each
//! produces **candidate chunk ids only** — enrichment
//! (`super::service::enrich_chunks`) is a separate step, which is what
//! lets all three input sources share one pipeline instead of three.
//!
//! None of the three ever fails in Node — every sub-effect is wrapped in
//! `Effect.catchAll(() => Effect.succeed([]))`, so the resolver's error
//! channel is literally `never`. `resolve_for_concept` and
//! `resolve_for_files` reproduce that: every fallible step degrades to "no
//! contribution from this step" rather than failing the whole call.
//! `resolve_for_plan` is the one exception — see its own doc comment.

use std::collections::{HashMap, HashSet};

use fubbik_core::error::{AppError, AppResult};
use fubbik_db::repo::{chunk, chunk_meta, plan, requirement, semantic};
use sqlx::PgPool;

// ---------------------------------------------------------------------------
// resolve_for_plan
// ---------------------------------------------------------------------------

/// Ports `resolveForPlan` (`resolvers.ts:139-177`): collects chunk ids from
/// a plan's `chunk`-kind analyze items, its linked requirements' chunks,
/// and its tasks' linked chunks.
///
/// **Divergence from Node, by deliberate ruling — read before "restoring
/// parity".** Node's `resolveForPlan(planId)` takes no `user_id` at all and
/// performs no ownership check: it queries `plan_analyze_item`/
/// `plan_requirement`/`plan_task` straight from `planId`
/// (`resolvers.ts:139-177`), relying entirely on `enrichChunks(ids,
/// userId)`'s per-chunk `getChunkById(id, userId)` filter three layers
/// downstream (`context/routes.ts:26-28`) to keep a cross-user request from
/// ever seeing another user's *chunk*. Nothing stops it from seeing another
/// user's *plan structure* — task titles, analyze-item text, which
/// requirements are linked — on the way there.
///
/// This port checks plan ownership up front instead, via `plan::find_by_id`
/// (the same guard `plans::service::get_plan` uses, `plans/service.rs:76`),
/// returning `NotFound` for a plan that isn't `user_id`'s. This mirrors
/// `fubbik_db::repo::plan`'s own module doc (divergence #13): "Every
/// function below takes `user_id` and filters on it in SQL — never left to
/// a caller-side check a future refactor could accidentally skip." Every
/// repo call this function makes (`list_analyze_items`, `list_requirements`,
/// `list_tasks`, `list_task_chunks_with_titles`, `requirement::get_chunks`)
/// already carries that same-shaped `EXISTS` ownership guard in SQL, so the
/// `find_by_id` check below is *also* redundant with each of them
/// individually — deliberately: this is defence in depth, not the only
/// layer. The chunk-level filter in `service::enrich_chunks` stays too.
/// **Do not delete either layer to "match `resolvers.ts`" — that is the
/// divergence, not a bug.**
pub async fn resolve_for_plan(
    pool: &PgPool,
    user_id: &str,
    plan_id: &str,
) -> AppResult<Vec<String>> {
    plan::find_by_id(pool, user_id, plan_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Plan".into()))?;

    let mut ids: HashSet<String> = HashSet::new();

    // 1. plan_analyze_item where kind = "chunk"
    let analyze_items = plan::list_analyze_items(pool, user_id, plan_id).await?;
    for item in analyze_items {
        if item.kind == "chunk"
            && let Some(chunk_id) = item.chunk_id
        {
            ids.insert(chunk_id);
        }
    }

    // 2. plan_requirement -> requirement_chunk
    let plan_reqs = plan::list_requirements(pool, user_id, plan_id).await?;
    for pr in plan_reqs {
        let chunks = requirement::get_chunks(pool, user_id, &pr.requirement_id).await?;
        for c in chunks {
            ids.insert(c.id);
        }
    }

    // 3. plan_task -> plan_task_chunk
    let tasks = plan::list_tasks(pool, user_id, plan_id).await?;
    for t in tasks {
        let task_chunks = plan::list_task_chunks_with_titles(pool, user_id, plan_id, &t.id).await?;
        for tc in task_chunks {
            ids.insert(tc.chunk_id);
        }
    }

    Ok(ids.into_iter().collect())
}

// ---------------------------------------------------------------------------
// resolve_for_concept
// ---------------------------------------------------------------------------

/// Ports `resolveForConcept` (`resolvers.ts:183-210`): semantic search plus
/// a text search, unioned. Never fails — see the module doc.
pub async fn resolve_for_concept(
    pool: &PgPool,
    ai: &fubbik_ai::OllamaClient,
    user_id: &str,
    query: &str,
    space_id: Option<&str>,
) -> AppResult<Vec<String>> {
    let mut ids: HashSet<String> = HashSet::new();

    // Semantic search — requires Ollama; falls back silently if the
    // embedding call or the search itself fails, matching Node's
    // `.pipe(Effect.catchAll(() => Effect.succeed([])))` at both steps.
    if let Ok(embedding) = ai.embed_query(query).await
        && let Ok(hits) =
            semantic::semantic_search(pool, &embedding, Some(user_id), &[], None, 20).await
    {
        for h in hits {
            ids.insert(h.id);
        }
    }

    // Text search over title/content.
    let params = chunk::ListParams {
        search: Some(query.to_string()),
        space_id: space_id.map(str::to_string),
        limit: 20,
        offset: 0,
        ..Default::default()
    };
    if let Ok(rows) = chunk::list(pool, user_id, &params).await {
        for c in rows {
            ids.insert(c.id);
        }
    }

    Ok(ids.into_iter().collect())
}

// ---------------------------------------------------------------------------
// resolve_for_files
// ---------------------------------------------------------------------------

/// Ports `resolveForFiles` (`resolvers.ts:216-235`), narrowed to the two
/// strategies its declared signature can actually reach.
///
/// **Scope divergence, not a partial port.** Node's `resolveForFiles`
/// delegates every path to the full `getContextForFile`
/// (`packages/api/src/context-for-file/service.ts`), a five-strategy
/// matcher: file-ref (+20), applies-to glob (+10), dependency (+3),
/// semantic (+5, needs Ollama), connected (+2). This function's signature —
/// `(pool, user_id, paths, space_id)`, carrying neither an `ai` client nor
/// a `deps` list — cannot reach the semantic or dependency strategies at
/// all, and "connected" only ever expands a result `getContextForFile`
/// already found. What remains reachable, and what this function
/// implements, is exactly the two highest-priority strategies: a direct
/// `chunk_file_ref` match on the literal path, and an `applies_to` glob
/// match. Porting the full five-strategy service is out of scope here —
/// it is a much larger, independently testable unit with its own Ollama
/// and dependency-matching concerns — and is left for whichever task takes
/// on `context-for-file` itself.
///
/// Never fails — see the module doc.
pub async fn resolve_for_files(
    pool: &PgPool,
    user_id: &str,
    paths: &[String],
    space_id: Option<&str>,
) -> AppResult<Vec<String>> {
    let mut ids: HashSet<String> = HashSet::new();

    // 1. Direct file-ref matches, one lookup per path.
    for path in paths {
        if let Ok(matches) =
            chunk_meta::lookup_chunk_ids_by_path(pool, path, user_id, space_id).await
        {
            for id in matches {
                ids.insert(id);
            }
        }
    }

    // 2. Applies-to glob matches, over chunks not already matched by step 1.
    let params = chunk::ListParams {
        space_id: space_id.map(str::to_string),
        limit: 1000,
        offset: 0,
        ..Default::default()
    };
    if let Ok(chunks) = chunk::list(pool, user_id, &params).await {
        let unchecked: Vec<String> = chunks
            .iter()
            .filter(|c| !ids.contains(&c.id))
            .map(|c| c.id.clone())
            .collect();

        if !unchecked.is_empty()
            && let Ok(patterns) = chunk_meta::get_applies_to_for_chunks(pool, &unchecked).await
        {
            let mut patterns_by_chunk: HashMap<String, Vec<String>> = HashMap::new();
            for p in patterns {
                patterns_by_chunk
                    .entry(p.chunk_id)
                    .or_default()
                    .push(p.pattern);
            }

            for c in &chunks {
                if ids.contains(&c.id) {
                    continue;
                }
                if let Some(pats) = patterns_by_chunk.get(&c.id)
                    && paths
                        .iter()
                        .any(|path| pats.iter().any(|pat| glob_match(pat, path)))
                {
                    ids.insert(c.id.clone());
                }
            }
        }
    }

    Ok(ids.into_iter().collect())
}

// ---------------------------------------------------------------------------
// glob_match — port of packages/api/src/context-for-file/glob-match.ts
// ---------------------------------------------------------------------------

/// Node builds this by turning the pattern into a `RegExp`
/// (`.replace(/\./g, "\\.").replace(/\*\*/g, ...).replace(/\*/g,
/// ...).replace(/\?/g, ...)`, anchored `^...$`). No `regex` crate is a
/// workspace dependency, and pulling one in for a single small matcher
/// would be a heavier footprint than a direct backtracking implementation
/// of the same four token classes (literal, `*`, `**`, `?`) the TS regex
/// encodes. `tokenize` mirrors the TS `.replace()` chain's left-to-right,
/// non-overlapping scan — `***` becomes `Globstar` then `Star`, exactly as
/// three sequential `.replace(/\*\*/g, ...)` passes would consume it.
fn normalize_path(path: &str) -> String {
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

fn glob_match(pattern: &str, path: &str) -> bool {
    let normalized_pattern = normalize_path(pattern);
    let normalized_path = normalize_path(path);
    let tokens = tokenize(&normalized_pattern);
    let text: Vec<char> = normalized_path.chars().collect();
    match_tokens(&tokens, &text)
}

#[cfg(test)]
mod glob_tests {
    use super::glob_match;

    #[test]
    fn exact_literal_path_matches_itself() {
        assert!(glob_match("src/foo.rs", "src/foo.rs"));
        assert!(!glob_match("src/foo.rs", "src/bar.rs"));
    }

    #[test]
    fn single_star_does_not_cross_a_slash() {
        assert!(glob_match("src/*.rs", "src/foo.rs"));
        assert!(!glob_match("src/*.rs", "src/nested/foo.rs"));
    }

    #[test]
    fn double_star_crosses_slashes() {
        assert!(glob_match("src/**/*.rs", "src/a/b/foo.rs"));
        assert!(glob_match(
            "src/**/*.rs",
            "src/foo.rs" /* zero-depth globstar */
        ));
    }

    #[test]
    fn leading_and_trailing_slashes_and_dot_slash_are_normalized_away() {
        assert!(glob_match("./src/foo.rs", "/src/foo.rs/"));
    }

    #[test]
    fn dot_is_literal_not_any_char() {
        assert!(!glob_match("src/foo.rs", "src/fooXrs"));
    }
}
