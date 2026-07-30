use sqlx::{Executor, PgPool, Row};

/// Escapes a value for use inside a Cypher single-quoted literal.
/// Backslashes must be escaped before quotes or the quote's escape
/// character gets doubled. Mirrors `escCypher` in the TS implementation.
pub fn esc_cypher(value: &str) -> String {
    value.replace('\\', r"\\").replace('\'', r"\'")
}

/// Reports whether the AGE extension is installed and the catalog readable.
pub async fn is_available(pool: &PgPool) -> bool {
    sqlx::query("SELECT 1 FROM ag_catalog.ag_graph LIMIT 0")
        .execute(pool)
        .await
        .is_ok()
}

/// Runs a Cypher query against the `knowledge` graph and returns each row
/// as JSON. Returns an empty vec when AGE is unavailable, matching the TS
/// behaviour of degrading rather than failing.
///
/// The `::text` cast is essential: sqlx has no decoder for `agtype`, so the
/// value must be stringified by Postgres before it crosses the wire.
///
/// # Safety
///
/// `query` is interpolated directly into a `$$`-dollar-quoted SQL statement
/// via `format!` — it is NOT parameterized. `esc_cypher` only escapes
/// backslashes and single quotes for Cypher string-literal safety; it does
/// NOT protect the surrounding `$$ ... $$` SQL dollar-quoting. A value
/// containing the literal substring `$$` can terminate the dollar-quoted
/// block early and inject arbitrary SQL. Callers must never build `query`
/// from untrusted input without additional sanitization (e.g. rejecting or
/// escaping `$$`). This flaw is inherited unchanged from the TypeScript
/// original (`packages/db/src/age/client.ts`) and is not addressed by this
/// helper.
pub async fn cypher(pool: &PgPool, query: &str) -> Result<Vec<serde_json::Value>, sqlx::Error> {
    if !is_available(pool).await {
        return Ok(Vec::new());
    }

    // `::varchar`, NOT `::text`. Verified against AGE 1.7.0: the explicit
    // text cast routes through agtype_value_to_text, which rejects vertex,
    // edge, and path values with "unsupported argument agtype 6". The
    // varchar coercion uses the type's output representation and handles
    // every shape. `agtype_out(v)` also produces the right string but
    // returns pseudo-type cstring, which sqlx cannot decode.
    let sql =
        format!("SELECT v::varchar AS v FROM cypher('knowledge', $$ {query} $$) AS (v agtype)");

    // A pooled connection is not guaranteed to have gone through
    // `connect()`'s `after_connect` hook — `#[sqlx::test]`-provisioned pools
    // bypass it entirely. AGE's `cypher()` function is unresolvable without
    // a session that has run `LOAD 'age'` (schema-qualifying the call is not
    // enough: `ag_catalog.cypher(...)` still fails with "unhandled
    // cypher(cstring) function call" if the library was never loaded).
    // Acquiring a single connection and priming it here, then running the
    // query on that same connection, makes `cypher()` self-sufficient
    // regardless of how the pool was built.
    let mut conn = pool.acquire().await?;
    conn.execute("LOAD 'age';").await?;
    conn.execute(r#"SET search_path = ag_catalog, "$user", public;"#)
        .await?;

    let rows = sqlx::query(&sql).fetch_all(&mut *conn).await?;

    Ok(rows
        .into_iter()
        .filter_map(|row| {
            let raw: String = row.try_get("v").ok()?;
            let parsed = parse_agtype(&raw);
            if parsed.is_none() {
                tracing::warn!(raw = %raw, "age::cypher: failed to parse agtype row, dropping it");
            }
            parsed
        })
        .collect())
}

/// Parses an agtype text representation into JSON.
///
/// Verified shapes from AGE 1.7.0:
///   vertex: {"id": 1125899906842625, "label": "chunk", "properties": {...}}::vertex
///   edge:   {"id": ..., "label": "REL", "start_id": ..., "end_id": ..., "properties": {...}}::edge
///   path:   [{...}::vertex, {...}::edge, {...}::vertex]::path
///   scalars: 42 | 1.5 | 1.5::numeric | "plain string"
///
/// AGE suffixes composite (and some scalar) values with `::identifier`. For a
/// path, every vertex/edge nested inside the array carries its OWN suffix in
/// addition to the outer `::path` suffix, which produces invalid JSON if only
/// the trailing suffix is stripped. `strip_type_suffixes` walks the whole
/// string and removes every `::identifier` occurrence that appears outside a
/// JSON string literal, so nested composites of arbitrary depth are handled
/// uniformly. Backslash-escaped characters inside strings (including escaped
/// quotes) are tracked so string boundaries are never miscounted, which is
/// what keeps property values that themselves contain `::` (e.g.
/// `{"code": "a::b"}`) intact.
fn parse_agtype(raw: &str) -> Option<serde_json::Value> {
    let trimmed = raw.trim();
    let stripped = strip_type_suffixes(trimmed);
    serde_json::from_str(&stripped).ok()
}

/// Removes every `::identifier` suffix (`::` followed by one or more ASCII
/// lowercase letters) that occurs outside a double-quoted JSON string,
/// tracking backslash escapes so escaped quotes don't wrongly toggle
/// in-string state.
fn strip_type_suffixes(raw: &str) -> String {
    let chars: Vec<char> = raw.chars().collect();
    let mut out = String::with_capacity(raw.len());
    let mut i = 0;
    let mut in_string = false;

    while i < chars.len() {
        let c = chars[i];

        if in_string {
            out.push(c);
            if c == '\\' && i + 1 < chars.len() {
                // Escaped char (e.g. `\"`, `\\`): copy it verbatim and don't
                // let it toggle `in_string` on the next iteration.
                out.push(chars[i + 1]);
                i += 2;
                continue;
            }
            if c == '"' {
                in_string = false;
            }
            i += 1;
            continue;
        }

        if c == '"' {
            in_string = true;
            out.push(c);
            i += 1;
            continue;
        }

        if c == ':' && chars.get(i + 1) == Some(&':') {
            let mut j = i + 2;
            while j < chars.len() && chars[j].is_ascii_lowercase() {
                j += 1;
            }
            if j > i + 2 {
                // Found a non-empty run of lowercase letters after `::`:
                // it's a type suffix, skip it entirely.
                i = j;
                continue;
            }
        }

        out.push(c);
        i += 1;
    }

    out
}

#[cfg(test)]
mod tests {
    use super::parse_agtype;

    #[test]
    fn strips_vertex_suffix() {
        // Exact output captured from AGE 1.7.0.
        let raw = r#"{"id": 1125899906842625, "label": "chunk", "properties": {"url": "https://x.test", "title": "hello"}}::vertex"#;
        let v = parse_agtype(raw).unwrap();
        assert_eq!(v["label"], "chunk");
        assert_eq!(v["properties"]["title"], "hello");
        assert_eq!(v["properties"]["url"], "https://x.test");
    }

    #[test]
    fn parses_bare_scalars() {
        assert_eq!(parse_agtype("42").unwrap(), 42);
        assert_eq!(parse_agtype("1.5").unwrap(), 1.5);
        assert_eq!(parse_agtype(r#""plain string""#).unwrap(), "plain string");
    }

    #[test]
    fn preserves_property_values_containing_double_colons() {
        let raw = r#"{"id": 1407374883553281, "label": "probe", "properties": {"code": "a::b"}}::vertex"#;
        let v = parse_agtype(raw).unwrap();
        assert_eq!(v["properties"]["code"], "a::b");
    }

    #[test]
    fn strips_edge_suffix() {
        let raw = r#"{"id": 2251799813685249, "label": "REL_REV", "end_id": 1125899906842625, "start_id": 1125899906842626, "properties": {}}::edge"#;
        let v = parse_agtype(raw).unwrap();
        assert_eq!(v["label"], "REL_REV");
        assert_eq!(v["start_id"], 1125899906842626_i64);
        assert_eq!(v["end_id"], 1125899906842625_i64);
    }

    #[test]
    fn strips_all_nested_suffixes_in_a_path() {
        // A path result: every vertex/edge nested in the array carries its
        // own `::vertex`/`::edge` suffix in ADDITION to the outer `::path`
        // suffix. Only stripping the trailing suffix leaves this invalid
        // JSON (this is the bug the scanner fixes).
        let raw = r#"[{"id": 1125899906842625, "label": "probe_rev", "properties": {}}::vertex, {"id": 2251799813685249, "label": "REL_REV", "end_id": 1125899906842627, "start_id": 1125899906842625, "properties": {}}::edge, {"id": 1125899906842627, "label": "probe_rev", "properties": {}}::vertex]::path"#;
        let v = parse_agtype(raw).unwrap();
        let arr = v.as_array().unwrap();
        assert_eq!(arr.len(), 3);
        assert_eq!(arr[0]["label"], "probe_rev");
        assert_eq!(arr[1]["label"], "REL_REV");
        assert_eq!(arr[2]["label"], "probe_rev");
    }

    #[test]
    fn strips_numeric_scalar_suffix() {
        assert_eq!(parse_agtype("1.5::numeric").unwrap(), 1.5);
    }
}
