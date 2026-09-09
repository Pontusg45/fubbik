use std::collections::HashMap;

use fubbik_core::error::AppResult;
use sqlx::{Acquire, Executor, PgPool, Row};

/// Escapes a value for use inside a Cypher single-quoted literal.
/// Backslashes must be escaped before quotes or the quote's escape
/// character gets doubled. Mirrors `escCypher` in the TS implementation.
pub fn esc_cypher(value: &str) -> String {
    value.replace('\\', r"\\").replace('\'', r"\'")
}

fn validate_identifier(value: &str, kind: &str) -> Result<(), sqlx::Error> {
    let mut chars = value.chars();
    let valid_start = chars
        .next()
        .is_some_and(|character| character == '_' || character.is_ascii_alphabetic());
    if valid_start && chars.all(|character| character == '_' || character.is_ascii_alphanumeric()) {
        Ok(())
    } else {
        Err(sqlx::Error::Protocol(format!(
            "invalid AGE {kind} identifier: {value:?}"
        )))
    }
}

/// Chooses a dollar-quote delimiter absent from the query. Unlike a fixed
/// `$$` delimiter, arbitrary user values cannot terminate this SQL literal.
fn dollar_quote(value: &str) -> String {
    for suffix in 0_u32.. {
        let delimiter = format!("$fubbik_{suffix}$");
        if !value.contains(&delimiter) {
            return format!("{delimiter}{value}{delimiter}");
        }
    }
    unreachable!("u32 delimiter space cannot be exhausted by an in-memory query")
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
/// Thin wrapper over [`cypher_in_graph`] fixed to the `"knowledge"` graph —
/// every caller in this module before Task 9 only ever needed that one
/// graph, so this signature is left untouched (two args, not three) rather
/// than threading a graph parameter through every existing call site in
/// `tests/age.rs`. [`get_neighborhood_in_graph`] is the one place that
/// genuinely needs a variable graph name (so its own degradation test can
/// point at a nonexistent graph), and it calls [`cypher_in_graph`] directly.
pub async fn cypher(pool: &PgPool, query: &str) -> Result<Vec<serde_json::Value>, sqlx::Error> {
    cypher_in_graph(pool, "knowledge", query).await
}

/// Multi-column sibling of [`cypher`], fixed to the `"knowledge"` graph.
/// Returns one `column name -> parsed value` map per row.
///
/// Prefer this over hand-parsing a single `v` column when a query `RETURN`s
/// several values: it inherits [`parse_agtype`], so quoted characters inside
/// property values survive. The TypeScript original stripped them with a raw
/// `String(v).replace(/"/g, "")` (`packages/api/src/graph/service.ts:86-127`),
/// silently corrupting any title containing a double quote.
pub async fn cypher_columns(
    pool: &PgPool,
    query: &str,
    columns: &[&str],
) -> Result<Vec<HashMap<String, serde_json::Value>>, sqlx::Error> {
    cypher_multi(pool, "knowledge", query, columns).await
}

/// The `graph`-parameterized core of [`cypher`]. Returns a single scalar
/// column (aliased `v`) per row, parsed via [`parse_agtype`].
///
/// The `::varchar` cast is essential: sqlx has no decoder for `agtype`, so
/// the value must be stringified by Postgres before it crosses the wire.
///
async fn cypher_in_graph(
    pool: &PgPool,
    graph: &str,
    query: &str,
) -> Result<Vec<serde_json::Value>, sqlx::Error> {
    if !is_available(pool).await {
        return Ok(Vec::new());
    }
    validate_identifier(graph, "graph")?;

    // `::varchar`, NOT `::text`. Verified against AGE 1.7.0: the explicit
    // text cast routes through agtype_value_to_text, which rejects vertex,
    // edge, and path values with "unsupported argument agtype 6". The
    // varchar coercion uses the type's output representation and handles
    // every shape. `agtype_out(v)` also produces the right string but
    // returns pseudo-type cstring, which sqlx cannot decode.
    let query = dollar_quote(query);
    let sql = format!("SELECT v::varchar AS v FROM cypher('{graph}', {query}) AS (v agtype)");

    let rows = run_primed(pool, &sql).await?;

    rows.into_iter()
        .map(|row| {
            let raw: String = row.try_get("v")?;
            parse_agtype(&raw)
                .ok_or_else(|| sqlx::Error::Protocol(format!("failed to parse AGE value: {raw}")))
        })
        .collect()
}

/// Multi-column sibling of [`cypher_in_graph`], for queries that `RETURN`
/// more than one value per row (e.g. `source`/`target`/`relation` triples).
/// Every named column is cast `::varchar` (same rationale as
/// [`cypher_in_graph`]) and parsed via [`parse_agtype`]; a row is returned
/// as a `column name -> parsed value` map so callers can pull out whichever
/// columns they asked for by name.
async fn cypher_multi(
    pool: &PgPool,
    graph: &str,
    query: &str,
    columns: &[&str],
) -> Result<Vec<HashMap<String, serde_json::Value>>, sqlx::Error> {
    if !is_available(pool).await {
        return Ok(Vec::new());
    }

    validate_identifier(graph, "graph")?;
    for column in columns {
        validate_identifier(column, "column")?;
    }
    let select_list = columns
        .iter()
        .map(|c| format!("{c}::varchar AS {c}"))
        .collect::<Vec<_>>()
        .join(", ");
    let column_defs = columns
        .iter()
        .map(|c| format!("{c} agtype"))
        .collect::<Vec<_>>()
        .join(", ");
    let query = dollar_quote(query);
    let sql = format!("SELECT {select_list} FROM cypher('{graph}', {query}) AS ({column_defs})");

    let rows = run_primed(pool, &sql).await?;

    rows.into_iter()
        .map(|row| {
            columns
                .iter()
                .map(|column| {
                    let raw: String = row.try_get(*column)?;
                    let parsed = parse_agtype(&raw).ok_or_else(|| {
                        sqlx::Error::Protocol(format!("failed to parse AGE column {column}: {raw}"))
                    })?;
                    Ok((column.to_string(), parsed))
                })
                .collect::<Result<HashMap<_, _>, sqlx::Error>>()
        })
        .collect()
}

/// Shared connection-priming plumbing for both [`cypher_in_graph`] and
/// [`cypher_multi`]: acquires a single pooled connection, primes it with
/// `LOAD 'age'`, runs `sql` inside a transaction with `search_path` scoped
/// via `SET LOCAL`, and returns the raw rows.
///
/// A pooled connection is not guaranteed to have gone through `connect()`'s
/// `after_connect` hook — `#[sqlx::test]`-provisioned pools bypass it
/// entirely. AGE's `cypher()` function is unresolvable without a session
/// that has run `LOAD 'age'` (schema-qualifying the call is not enough:
/// `ag_catalog.cypher(...)` still fails with "unhandled cypher(cstring)
/// function call" if the library was never loaded). Acquiring a single
/// connection and priming it here, then running the query on that same
/// connection, makes `cypher()` self-sufficient regardless of how the pool
/// was built.
///
/// `SET search_path` (session-scoped) would persist on this connection
/// after it is returned to the pool — sqlx runs no `DISCARD ALL` on
/// release. That is exactly the mechanism that once broke
/// `sqlx::migrate!`'s bookkeeping-table resolution (see the comment on
/// `connect`'s `after_connect` hook in `lib.rs`), just one step removed:
/// instead of every connection starting with the mutation, a single
/// connection returns to the pool carrying it, and whichever caller
/// acquires that connection next inherits it silently. `SET LOCAL` inside a
/// transaction is scoped to that transaction only — it reverts
/// automatically on COMMIT or ROLLBACK, including on the error path via
/// `?`, so the connection can never leave this function with a mutated
/// search_path.
async fn run_primed(pool: &PgPool, sql: &str) -> Result<Vec<sqlx::postgres::PgRow>, sqlx::Error> {
    let mut conn = pool.acquire().await?;
    conn.execute("LOAD 'age';").await?;

    let mut tx = conn.begin().await?;
    sqlx::query(r#"SET LOCAL search_path = ag_catalog, "$user", public;"#)
        .execute(&mut *tx)
        .await?;

    let rows = sqlx::query(sql).fetch_all(&mut *tx).await?;
    tx.commit().await?;

    Ok(rows)
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

// ---------------------------------------------------------------------------
// Seed helpers (test-only in practice, but not `#[cfg(test)]`-gated: the
// integration tests in `tests/age_queries.rs` need them as ordinary crate
// items, the same way `tests/age.rs` calls `age::cypher` directly).
// ---------------------------------------------------------------------------

/// `MERGE (:chunk {id: '...'})` — idempotent vertex creation for a `chunk`
/// vertex. Matches Node's `ensureVertex("chunk", id)`
/// (`packages/db/src/repository/chunk.ts:306`), hardcoded to the `chunk`
/// label since that is the only vertex kind this port's query helpers (and
/// their tests) need to seed.
pub async fn ensure_vertex(pool: &PgPool, chunk_id: &str) -> AppResult<()> {
    cypher(
        pool,
        &format!("MERGE (:chunk {{id: '{}'}})", esc_cypher(chunk_id)),
    )
    .await?;
    Ok(())
}

/// Creates a `:connects` edge between two `chunk` vertices, carrying
/// `relation` as an edge property — the real shape Node's connections
/// repository writes (`packages/db/src/repository/connection.ts:22`:
/// `createEdge("connects", "chunk", sourceId, "chunk", targetId, { id,
/// relation })`, minus the `id` property, which nothing in this module's
/// query helpers reads).
///
/// **`MERGE`, not `CREATE`.** The whole pattern — both endpoints AND the
/// `{relation: '...'}` property — is the merge key, so re-running this for
/// the same `(from_id, to_id, relation)` triple is a no-op instead of
/// stacking a second parallel edge; this is what makes
/// [`backfill_connections`] safe to run more than once (Task 10's whole
/// justification for `MERGE` over `CREATE`). A *different* `relation`
/// between the same two vertices still creates a distinct edge, matching
/// `chunk_connection`'s own `(source_id, target_id, relation)` unique index
/// — each connection row is its own real-world edge.
pub async fn create_edge(
    pool: &PgPool,
    relation: &str,
    from_id: &str,
    to_id: &str,
) -> AppResult<()> {
    let query = format!(
        "MATCH (a:chunk {{id: '{}'}}), (b:chunk {{id: '{}'}}) MERGE (a)-[:connects {{relation: '{}'}}]->(b)",
        esc_cypher(from_id),
        esc_cypher(to_id),
        esc_cypher(relation)
    );
    cypher(pool, &query).await?;
    Ok(())
}

/// Removes the `:connects` edge carrying this exact `relation` property
/// between two `chunk` vertices — the inverse of [`create_edge`], called
/// after `chunk_connection`'s row is deleted (`connection::delete`). Scoped
/// to the specific `relation`, not "any edge between these two vertices":
/// if multiple relation types exist between the same pair (each backed by
/// its own `chunk_connection` row), removing one must not touch the
/// others.
///
/// A `MATCH ... DELETE` with no matching edge (already gone, vertices
/// never projected, AGE unavailable) is simply a no-op — same "missing is
/// fine" tolerance [`ensure_vertex`]/[`create_edge`] have via `MERGE`.
pub async fn delete_edge(
    pool: &PgPool,
    relation: &str,
    from_id: &str,
    to_id: &str,
) -> AppResult<()> {
    let query = format!(
        "MATCH (a:chunk {{id: '{}'}})-[e:connects {{relation: '{}'}}]->(b:chunk {{id: '{}'}}) DELETE e",
        esc_cypher(from_id),
        esc_cypher(relation),
        esc_cypher(to_id)
    );
    cypher(pool, &query).await?;
    Ok(())
}

/// Counts `:connects` edges from `a` to `b`, irrespective of the
/// `relation` property — a test helper for proving [`backfill_connections`]
/// is idempotent (`count_edges_between` must stay `1` after the backfill
/// runs twice over the same source row, not climb to `2`).
pub async fn count_edges_between(pool: &PgPool, a: &str, b: &str) -> AppResult<i64> {
    let query = format!(
        "MATCH (x:chunk {{id: '{}'}})-[e:connects]->(y:chunk {{id: '{}'}}) RETURN count(e) AS c",
        esc_cypher(a),
        esc_cypher(b)
    );
    let rows = cypher(pool, &query).await?;
    Ok(rows.first().and_then(|v| v.as_i64()).unwrap_or(0))
}

/// One-time, idempotent walk of `chunk_connection`, projecting every row
/// into the AGE graph via [`ensure_vertex`]/[`create_edge`] — the backfill
/// for rows that existed before this port started projecting connections
/// on write. Ordered by `id` purely for deterministic, reproducible runs;
/// nothing about correctness depends on the order.
///
/// Returns the number of rows walked (processed), **not** the number of
/// edges newly created — every row is re-projected on every call, and
/// `create_edge`'s `MERGE` is what makes that safe to repeat: running this
/// twice against the same data returns the same count both times, and
/// [`count_edges_between`] confirms the edge set itself did not grow.
///
/// Never called from the server's startup path — exposed only as an
/// explicit `fubbik` CLI subcommand (`crates/fubbik/src/main.rs`). An
/// implicit graph rewrite at boot is exactly the kind of surprise this is
/// meant to avoid.
pub async fn backfill_connections(pool: &PgPool) -> AppResult<u64> {
    let rows =
        sqlx::query!("SELECT source_id, target_id, relation FROM chunk_connection ORDER BY id")
            .fetch_all(pool)
            .await?;

    let mut count = 0u64;
    for row in &rows {
        ensure_vertex(pool, &row.source_id).await?;
        ensure_vertex(pool, &row.target_id).await?;
        create_edge(pool, &row.relation, &row.source_id, &row.target_id).await?;
        count += 1;
    }
    Ok(count)
}

// ---------------------------------------------------------------------------
// Query helpers wired into search (Task 9) and staleness's scan-impact.
// ---------------------------------------------------------------------------

/// Extracts a JSON string value out of an already-parsed agtype row.
fn as_string(v: Option<&serde_json::Value>) -> Option<String> {
    v.and_then(|v| v.as_str()).map(str::to_string)
}

/// The `graph`-parameterized core query behind [`get_neighborhood`]. The
/// `graph` parameter exists purely so the degradation test can point at a
/// nonexistent graph, simulating AGE being unavailable — every real caller
/// goes through [`get_neighborhood`], which always passes `"knowledge"`.
///
/// Degrades to `Ok(vec![])` on any query failure (nonexistent graph,
/// nonexistent chunk vertex, AGE extension missing, etc.), never
/// propagating an error — Node wraps every graph clause in
/// `Effect.orElse(() => Effect.succeed([]))`
/// (`packages/api/src/search/service.ts:92`), and an unavailable graph must
/// degrade the same way here.
pub async fn get_neighborhood_in_graph(
    pool: &PgPool,
    graph: &str,
    chunk_id: &str,
    hops: i32,
) -> AppResult<Vec<String>> {
    let query = format!(
        "MATCH (a:chunk {{id: '{}'}})-[*1..{}]-(b:chunk) RETURN DISTINCT b.id",
        esc_cypher(chunk_id),
        hops
    );
    match cypher_in_graph(pool, graph, &query).await {
        Ok(rows) => Ok(rows.iter().filter_map(|v| as_string(Some(v))).collect()),
        Err(_) => Ok(vec![]),
    }
}

/// Port of Node's `getNeighborhood` (`packages/db/src/age/query.ts:37-43`):
/// every `chunk` reachable from `chunk_id` within `hops` hops, in either
/// direction, against the real `"knowledge"` graph.
pub async fn get_neighborhood(pool: &PgPool, chunk_id: &str, hops: i32) -> AppResult<Vec<String>> {
    get_neighborhood_in_graph(pool, "knowledge", chunk_id, hops).await
}

/// Ports `getConnectionDegrees` (`packages/db/src/age/query.ts:143-163`):
/// the number of graph edges touching each of `chunk_ids`, for
/// `context_for_file::service`'s centrality-boosted scoring.
///
/// Degrades to an empty map on any failure — a nonexistent graph, a query
/// error, or (the case that matters in this deployment) AGE not being
/// installed at all: [`cypher_columns`] -> [`cypher_multi`] already checks
/// [`is_available`] first and returns `Ok(vec![])` rather than erroring, so
/// this function's `unwrap_or_default()` is defence in depth, not the only
/// thing standing between an unavailable AGE extension and a 500. Matches
/// Node's own two-layer fallback: `getConnectionDegrees` itself has no
/// `catchAll`, but every caller wraps it in one
/// (`context-for-file/service.ts:262`).
pub async fn get_connection_degrees(pool: &PgPool, chunk_ids: &[String]) -> HashMap<String, i64> {
    if chunk_ids.is_empty() {
        return HashMap::new();
    }
    let id_list = chunk_ids
        .iter()
        .map(|id| format!("'{}'", esc_cypher(id)))
        .collect::<Vec<_>>()
        .join(",");
    let query = format!(
        "MATCH (c:chunk)-[e]-() WHERE c.id IN [{id_list}] RETURN c.id AS id, count(e) AS degree"
    );
    let rows = cypher_columns(pool, &query, &["id", "degree"])
        .await
        .unwrap_or_default();

    let mut map = HashMap::new();
    for row in rows {
        let id = row.get("id").and_then(|v| v.as_str()).map(str::to_string);
        let degree = row.get("degree").and_then(|v| v.as_i64());
        if let (Some(id), Some(degree)) = (id, degree) {
            map.insert(id, degree);
        }
    }
    map
}

/// Ports `getGraphProximityBoost` (`packages/db/src/age/query.ts:166-187`):
/// for each of `candidate_ids` reachable from `anchor_id` within `max_hops`,
/// `1 / hops` — a hybrid boost applied to semantic matches that are also
/// graph-close to a high-confidence anchor (a file-ref or applies-to hit).
///
/// Same two-layer degradation as [`get_connection_degrees`]: `cypher_columns`
/// already returns `Ok(vec![])` when AGE is unavailable, and this function's
/// `unwrap_or_default()` also absorbs a genuine query error (e.g. no path
/// exists within `max_hops`, which is not an error condition in Cypher but
/// is handled identically either way — an empty map, not a panic).
pub async fn get_graph_proximity_boost(
    pool: &PgPool,
    anchor_id: &str,
    candidate_ids: &[String],
    max_hops: i64,
) -> HashMap<String, f64> {
    if candidate_ids.is_empty() {
        return HashMap::new();
    }
    let id_list = candidate_ids
        .iter()
        .map(|id| format!("'{}'", esc_cypher(id)))
        .collect::<Vec<_>>()
        .join(",");
    let query = format!(
        "MATCH (anchor:chunk {{id: '{}'}}), (target:chunk) \
         WHERE target.id IN [{id_list}] \
         MATCH p = shortestPath((anchor)-[*1..{max_hops}]-(target)) \
         RETURN target.id AS id, length(p) AS hops",
        esc_cypher(anchor_id)
    );
    let rows = cypher_columns(pool, &query, &["id", "hops"])
        .await
        .unwrap_or_default();

    let mut map = HashMap::new();
    for row in rows {
        let id = row.get("id").and_then(|v| v.as_str()).map(str::to_string);
        let hops = row.get("hops").and_then(|v| v.as_i64());
        if let (Some(id), Some(hops)) = (id, hops)
            && hops > 0
        {
            map.insert(id, 1.0 / hops as f64);
        }
    }
    map
}

/// One edge on a resolved path, matching Node's `PathEdge`
/// (`packages/db/src/age/query.ts:322-326`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathEdgeInfo {
    pub source: String,
    pub target: String,
    pub relation: String,
}

/// The result of [`find_shortest_path_with_details`]: the chain of chunk
/// ids from source to target (inclusive of both endpoints) plus the edges
/// connecting each consecutive pair.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PathDetails {
    pub chunk_ids: Vec<String>,
    pub edges: Vec<PathEdgeInfo>,
}

/// Port of Node's `findShortestPathWithDetails`
/// (`packages/db/src/age/query.ts:334-411`). Two queries, not one: AGE 1.x
/// has no `shortestPath()`/list-comprehension support strong enough to do
/// this in a single Cypher statement (see Node's own comment on
/// `findShortestPath`), so Node — and this port — first checks reachability
/// with a bounded variable-length traversal, then fetches every `:connects`
/// edge in the whole graph and runs a plain BFS in application code to
/// reconstruct the actual path and its edges.
///
/// Degrades to `Ok(None)` on any query failure, matching Node's
/// `Effect.catchAll(() => Effect.succeed(null))`
/// (`packages/db/src/age/query.ts:409`) — the same "no path" result a
/// caller gets when one genuinely doesn't exist, which is what lets the
/// search service's `Effect.orElse` wrapper at the call site
/// (`service.ts:99`) be redundant-but-harmless rather than load-bearing.
///
/// Thin wrapper over [`find_shortest_path_with_details_in_graph`] fixed to
/// the `"knowledge"` graph, same shape as [`get_neighborhood`] over
/// [`get_neighborhood_in_graph`].
pub async fn find_shortest_path_with_details(
    pool: &PgPool,
    from: &str,
    to: &str,
) -> AppResult<Option<PathDetails>> {
    find_shortest_path_with_details_in_graph(pool, "knowledge", from, to).await
}

/// The `graph`-parameterized core of [`find_shortest_path_with_details`].
/// The `graph` parameter exists purely so its degradation test can point at
/// a nonexistent graph, simulating AGE being unavailable — every real
/// caller goes through [`find_shortest_path_with_details`], which always
/// passes `"knowledge"`.
pub async fn find_shortest_path_with_details_in_graph(
    pool: &PgPool,
    graph: &str,
    from: &str,
    to: &str,
) -> AppResult<Option<PathDetails>> {
    let check_query = format!(
        "MATCH (a:chunk {{id: '{}'}})-[*1..10]-(b:chunk {{id: '{}'}}) RETURN b.id AS found LIMIT 1",
        esc_cypher(from),
        esc_cypher(to)
    );
    let reachable = match cypher_in_graph(pool, graph, &check_query).await {
        Ok(rows) => !rows.is_empty(),
        Err(_) => return Ok(None),
    };
    if !reachable {
        return Ok(None);
    }

    let edges_query = "MATCH (x:chunk)-[e:connects]->(y:chunk) RETURN x.id AS source, y.id AS target, e.relation AS relation";
    let rows = match cypher_multi(pool, graph, edges_query, &["source", "target", "relation"]).await
    {
        Ok(rows) => rows,
        Err(_) => return Ok(None),
    };

    // Undirected adjacency: a `:connects` edge is matched in either
    // direction by the reachability check above, so the BFS must be able
    // to traverse it both ways too.
    let mut adjacency: HashMap<String, Vec<(String, String)>> = HashMap::new();
    for row in &rows {
        let source = as_string(row.get("source"));
        let target = as_string(row.get("target"));
        let relation = as_string(row.get("relation"));
        let (Some(s), Some(t), Some(r)) = (source, target, relation) else {
            continue;
        };
        adjacency
            .entry(s.clone())
            .or_default()
            .push((t.clone(), r.clone()));
        adjacency.entry(t).or_default().push((s, r));
    }

    let mut visited: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut parent: HashMap<String, Option<(String, String)>> = HashMap::new();
    let mut queue: std::collections::VecDeque<String> = std::collections::VecDeque::new();
    visited.insert(from.to_string());
    parent.insert(from.to_string(), None);
    queue.push_back(from.to_string());

    while let Some(current) = queue.pop_front() {
        if current == to {
            break;
        }
        if let Some(neighbors) = adjacency.get(&current) {
            for (neighbor, relation) in neighbors {
                if !visited.contains(neighbor) {
                    visited.insert(neighbor.clone());
                    parent.insert(neighbor.clone(), Some((current.clone(), relation.clone())));
                    queue.push_back(neighbor.clone());
                }
            }
        }
    }

    if !parent.contains_key(to) {
        return Ok(None);
    }

    let mut chunk_ids = Vec::new();
    let mut edges = Vec::new();
    let mut cursor = Some(to.to_string());
    while let Some(c) = cursor {
        chunk_ids.push(c.clone());
        match parent.get(&c).cloned().flatten() {
            Some((from_node, relation)) => {
                edges.push(PathEdgeInfo {
                    source: from_node.clone(),
                    target: c,
                    relation,
                });
                cursor = Some(from_node);
            }
            None => cursor = None,
        }
    }
    chunk_ids.reverse();
    edges.reverse();

    Ok(Some(PathDetails { chunk_ids, edges }))
}

/// Port of Node's `getChunksAffectedByRequirement`
/// (`packages/db/src/age/query.ts:90-96`): every chunk within `hops` hops
/// of any chunk a requirement `:covers`, including the covered chunks
/// themselves. Degrades to `Ok(vec![])` if even the (always-required)
/// covered-chunks query fails; the hop-traversal half degrades silently on
/// its own failure, keeping whatever covered-chunk ids were already found.
///
/// **Does not port Node's single `*0..hops` pattern**
/// (`MATCH (r)-[:covers]->(c)-[:connects*0..hops]-(related) RETURN
/// related.id`). Verified directly against this workspace's AGE 1.7.0: a
/// variable-length pattern with a **zero** lower bound on a relationship
/// label that has never been used anywhere in the graph yet fails to match
/// even the zero-hop case (the anchor node itself) — a `#[sqlx::test]`
/// pool is a fresh database with a fresh `"knowledge"` graph, so a chunk
/// with no existing `:connects` edges reproduces this every time. Splitting
/// the "covered chunks" (`*0`, unconditional, no `:connects` pattern
/// involved at all) from the "hop traversal" (`*1..hops`, only run when
/// `hops > 0`) sidesteps the bug instead of relying on the broken
/// zero-bound form.
///
/// Thin wrapper over [`get_chunks_affected_by_requirement_in_graph`] fixed
/// to the `"knowledge"` graph, same shape as [`get_neighborhood`] over
/// [`get_neighborhood_in_graph`].
pub async fn get_chunks_affected_by_requirement(
    pool: &PgPool,
    requirement_id: &str,
    hops: i32,
) -> AppResult<Vec<String>> {
    get_chunks_affected_by_requirement_in_graph(pool, "knowledge", requirement_id, hops).await
}

/// The `graph`-parameterized core of [`get_chunks_affected_by_requirement`].
/// The `graph` parameter exists purely so its degradation test can point at
/// a nonexistent graph, simulating AGE being unavailable — every real
/// caller goes through [`get_chunks_affected_by_requirement`], which always
/// passes `"knowledge"`.
pub async fn get_chunks_affected_by_requirement_in_graph(
    pool: &PgPool,
    graph: &str,
    requirement_id: &str,
    hops: i32,
) -> AppResult<Vec<String>> {
    let escaped = esc_cypher(requirement_id);

    let covers_query = format!(
        "MATCH (r:requirement {{id: '{escaped}'}})-[:covers]->(c:chunk) RETURN DISTINCT c.id AS id"
    );
    let mut ids: std::collections::HashSet<String> =
        match cypher_in_graph(pool, graph, &covers_query).await {
            Ok(rows) => rows.iter().filter_map(|v| as_string(Some(v))).collect(),
            Err(_) => return Ok(vec![]),
        };

    if hops > 0 {
        let related_query = format!(
            "MATCH (r:requirement {{id: '{escaped}'}})-[:covers]->(c:chunk)-[:connects*1..{hops}]-(related:chunk) \
             RETURN DISTINCT related.id AS id"
        );
        match cypher_in_graph(pool, graph, &related_query).await {
            Ok(rows) => ids.extend(rows.iter().filter_map(|v| as_string(Some(v)))),
            Err(err) => tracing::warn!(
                error = %err,
                "get_chunks_affected_by_requirement: hop-traversal query failed, keeping only the covered chunks already found"
            ),
        }
    }

    let mut result: Vec<String> = ids.into_iter().collect();
    result.sort();
    Ok(result)
}

/// Relation-type weights from Node's `RELATION_WEIGHT`
/// (`packages/db/src/age/impact.ts:9-15`) — a relation not in this table
/// (including AGE's own `"connects"` edge label, if a caller ever seeds an
/// edge with no `relation` property set) falls back to the same `0.2`
/// Node's own object-index-miss (`RELATION_WEIGHT[rel] ?? 0.2`) produces.
fn relation_weight(relation: &str) -> f64 {
    match relation {
        "depends_on" => 1.0,
        "extends" => 0.8,
        "part_of" => 0.7,
        "references" => 0.3,
        "related_to" => 0.2,
        _ => 0.2,
    }
}

/// Distance-decay weights from Node's `DISTANCE_DECAY`
/// (`packages/db/src/age/impact.ts:7`) — any hop count outside `{1,2,3}`
/// (impossible given the query's own `*1..3` bound, but mirrored for
/// parity) falls back to `0.1`, matching `DISTANCE_DECAY[hops] ?? 0.1`.
fn distance_decay(hops: i64) -> f64 {
    match hops {
        1 => 0.9,
        2 => 0.5,
        3 => 0.2,
        _ => 0.1,
    }
}

/// Port of Node's `computeImpactRipple`
/// (`packages/db/src/age/impact.ts:24-58`): every chunk downstream of
/// `chunk_id` within 3 hops along `:connects` edges, weighted by hop
/// distance and the *weakest* relation type on the path, keeping only the
/// best (highest-degree) score per downstream chunk and dropping any chunk
/// whose best degree doesn't clear `0.1`.
///
/// Returns only the surviving chunk ids — not Node's richer
/// `{chunkId, degree, hops, path}[]` — because nothing in this port reads
/// the degree/hops/path breakdown outside the (also simplified) staleness
/// detail message `staleness::flag_impact_ripple` writes; see that
/// function's doc comment.
///
/// **Does not port Node's exact Cypher.** Node's query
/// (`impact.ts:26-30`) does `RETURN ..., length(r) AS hops, [rel IN r |
/// rel.relation] AS path` — both `length()` over a variable-length
/// relationship list and the `[rel IN r | rel.relation]` list
/// comprehension fail against this workspace's AGE 1.7.0 with `length()
/// argument must resolve to a scalar` / `could not find properties for
/// rel` respectively (verified directly against `fubbik-rs-db`, not a
/// guess). This port instead returns the raw relationship list `r` and
/// derives `hops` (`.len()`) and each edge's `relation` property in Rust
/// after parsing — same inputs, same weighting formula below, no AGE
/// version-specific Cypher feature required.
///
/// Thin wrapper over [`compute_impact_ripple_in_graph`] fixed to the
/// `"knowledge"` graph, same shape as [`get_neighborhood`] over
/// [`get_neighborhood_in_graph`].
pub async fn compute_impact_ripple(pool: &PgPool, chunk_id: &str) -> AppResult<Vec<String>> {
    compute_impact_ripple_in_graph(pool, "knowledge", chunk_id).await
}

/// The `graph`-parameterized core of [`compute_impact_ripple`]. The `graph`
/// parameter exists purely so its degradation test can point at a
/// nonexistent graph, simulating AGE being unavailable — every real caller
/// goes through [`compute_impact_ripple`], which always passes
/// `"knowledge"`.
pub async fn compute_impact_ripple_in_graph(
    pool: &PgPool,
    graph: &str,
    chunk_id: &str,
) -> AppResult<Vec<String>> {
    let escaped = esc_cypher(chunk_id);
    let query = format!(
        "MATCH (source:chunk {{id: '{escaped}'}})-[r:connects*1..3]->(downstream:chunk) \
         WHERE downstream.id <> '{escaped}' \
         RETURN downstream.id AS did, r AS path"
    );
    let rows = match cypher_multi(pool, graph, &query, &["did", "path"]).await {
        Ok(rows) => rows,
        Err(_) => return Ok(vec![]),
    };

    let mut best: HashMap<String, f64> = HashMap::new();
    for row in &rows {
        let Some(did) = as_string(row.get("did")) else {
            continue;
        };
        let edges = row.get("path").and_then(|v| v.as_array());
        let Some(edges) = edges else {
            continue;
        };
        let hops = edges.len() as i64;
        let relations: Vec<String> = edges
            .iter()
            .filter_map(|e| e.get("properties")?.get("relation")?.as_str())
            .map(str::to_string)
            .collect();

        let distance_factor = distance_decay(hops);
        let relation_factor = relations
            .iter()
            .map(|r| relation_weight(r))
            .fold(1.0_f64, f64::min);
        let degree = distance_factor * relation_factor;

        if degree <= 0.1 {
            continue;
        }

        best.entry(did)
            .and_modify(|d| {
                if degree > *d {
                    *d = degree;
                }
            })
            .or_insert(degree);
    }

    let mut ids: Vec<String> = best.into_keys().collect();
    ids.sort();
    Ok(ids)
}

// ---------------------------------------------------------------------------
// Behavior rules
// ---------------------------------------------------------------------------
//
// Ports `packages/api/src/matrices/graph-sync.ts`. Kept here rather than in
// `fubbik-api` for the same reason `get_chunks_affected_by_requirement` is
// here: hand-written Cypher belongs to the AGE layer, and the service above
// should not be assembling query strings.

/// A `behavior_rule` vertex as the graph stores it.
#[derive(Debug, Clone, PartialEq, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BehaviorRuleVertex {
    pub id: String,
    pub title: String,
    pub layer: String,
    pub matrix_id: String,
    pub category: String,
}

/// A `governs` edge from a rule to the code it controls.
#[derive(Debug, Clone, PartialEq, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GovernsEdge {
    pub source_id: String,
    pub target_id: String,
    /// `file | symbol` — which vertex label the target is.
    pub kind: String,
}

/// `MERGE` the vertex on `id`, then `SET` its properties.
///
/// Two statements, not one `MERGE ... SET`, mirroring Node
/// (`graph-sync.ts:44-50`). AGE's `MERGE` support is the shakiest corner of
/// its Cypher implementation, and the split form is the one already proven to
/// work here by [`ensure_vertex`].
pub async fn upsert_behavior_rule(pool: &PgPool, rule: &BehaviorRuleVertex) -> AppResult<()> {
    cypher(
        pool,
        &format!("MERGE (:behavior_rule {{id: '{}'}})", esc_cypher(&rule.id)),
    )
    .await?;
    cypher(
        pool,
        &format!(
            "MATCH (r:behavior_rule {{id: '{}'}}) \
             SET r.title = '{}', r.layer = '{}', r.matrixId = '{}', r.category = '{}'",
            esc_cypher(&rule.id),
            esc_cypher(&rule.title),
            esc_cypher(&rule.layer),
            esc_cypher(&rule.matrix_id),
            esc_cypher(&rule.category),
        ),
    )
    .await?;
    Ok(())
}

/// Removes every `governs` edge leaving this rule, so the caller can rebuild
/// them. Deleting before relinking is what makes the sweep idempotent — the
/// alternative, `MERGE`-ing each edge, leaves edges behind for cell-code links
/// that were since deleted.
pub async fn delete_governs_edges(pool: &PgPool, rule_id: &str) -> AppResult<()> {
    cypher(
        pool,
        &format!(
            "MATCH (r:behavior_rule {{id: '{}'}})-[e:governs]->() DELETE e",
            esc_cypher(rule_id)
        ),
    )
    .await?;
    Ok(())
}

/// Links a rule to the code vertex whose id ends with `code_ref`.
///
/// `ENDS WITH` rather than `=` because `behavior_cell_code.ref` holds a
/// repo-relative path while `code_file.id` is absolute (`graph-sync.ts:57`).
/// A no-op when no such vertex exists, which is the normal state: `code-index`
/// is not ported, so nothing writes `code_file` or `code_symbol` vertices. See
/// the spec's "Nothing renders code or concept nodes".
pub async fn link_governs(
    pool: &PgPool,
    rule_id: &str,
    kind: &str,
    code_ref: &str,
) -> AppResult<()> {
    let label = if kind == "symbol" {
        "code_symbol"
    } else {
        "code_file"
    };
    cypher(
        pool,
        &format!(
            "MATCH (r:behavior_rule {{id: '{}'}}), (c:{label}) \
             WHERE c.id ENDS WITH '{}' \
             MERGE (r)-[:governs {{kind: '{}'}}]->(c)",
            esc_cypher(rule_id),
            esc_cypher(code_ref),
            esc_cypher(kind),
        ),
    )
    .await?;
    Ok(())
}

/// Every `behavior_rule` vertex. Degrades to empty when AGE is unavailable.
pub async fn list_behavior_rule_vertices(pool: &PgPool) -> AppResult<Vec<BehaviorRuleVertex>> {
    let rows = cypher_columns(
        pool,
        "MATCH (r:behavior_rule) \
         RETURN r.id AS id, r.title AS title, r.layer AS layer, \
                r.matrixId AS matrix_id, r.category AS category",
        &["id", "title", "layer", "matrix_id", "category"],
    )
    .await?;

    Ok(rows
        .into_iter()
        .filter_map(|row| {
            Some(BehaviorRuleVertex {
                id: row.get("id")?.as_str()?.to_string(),
                title: row
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                layer: row
                    .get("layer")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                matrix_id: row
                    .get("matrix_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                category: row
                    .get("category")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
            })
        })
        .collect())
}

/// Every `governs` edge. Degrades to empty when AGE is unavailable.
pub async fn list_governs_edges(pool: &PgPool) -> AppResult<Vec<GovernsEdge>> {
    let rows = cypher_columns(
        pool,
        "MATCH (r:behavior_rule)-[g:governs]->(c) \
         RETURN r.id AS source_id, c.id AS target_id, g.kind AS kind",
        &["source_id", "target_id", "kind"],
    )
    .await?;

    Ok(rows
        .into_iter()
        .filter_map(|row| {
            Some(GovernsEdge {
                source_id: row.get("source_id")?.as_str()?.to_string(),
                target_id: row.get("target_id")?.as_str()?.to_string(),
                kind: row
                    .get("kind")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
            })
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::{dollar_quote, parse_agtype, validate_identifier};

    #[test]
    fn dollar_quote_chooses_a_delimiter_absent_from_the_query() {
        assert_eq!(dollar_quote("RETURN 1"), "$fubbik_0$RETURN 1$fubbik_0$");
        assert_eq!(
            dollar_quote("RETURN '$fubbik_0$'"),
            "$fubbik_1$RETURN '$fubbik_0$'$fubbik_1$"
        );
    }

    #[test]
    fn age_identifiers_reject_sql_syntax() {
        assert!(validate_identifier("knowledge_2", "graph").is_ok());
        assert!(validate_identifier("knowledge'); DROP TABLE plan; --", "graph").is_err());
        assert!(validate_identifier("two columns", "column").is_err());
    }

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
        let raw =
            r#"{"id": 1407374883553281, "label": "probe", "properties": {"code": "a::b"}}::vertex"#;
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
