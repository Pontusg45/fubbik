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
            parse_agtype(&raw)
        })
        .collect())
}

/// Parses an agtype text representation into JSON.
///
/// Verified shapes from AGE 1.7.0:
///   vertex: {"id": 1125899906842625, "label": "chunk", "properties": {...}}::vertex
///   scalars: 42 | 1.5 | "plain string"   (no suffix)
///
/// Only a trailing `::identifier` is stripped. Matching the suffix by its
/// shape rather than by the last `::` in the string keeps property values
/// that themselves contain `::` (e.g. {"code": "a::b"}) from being mangled.
fn parse_agtype(raw: &str) -> Option<serde_json::Value> {
    let trimmed = raw.trim();

    let body = match trimmed.rfind("::") {
        Some(idx)
            if trimmed[idx + 2..].chars().all(|c| c.is_ascii_lowercase())
                && idx + 2 < trimmed.len() =>
        {
            &trimmed[..idx]
        }
        _ => trimmed,
    };

    serde_json::from_str(body).ok()
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
}
