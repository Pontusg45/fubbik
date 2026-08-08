use fubbik_db::repo::chunk::{Chunk, Enrichment, Sort};

#[derive(serde::Deserialize, utoipa::ToSchema)]
pub struct CreateChunkBody {
    pub title: String,
    #[serde(default)]
    pub content: String,
    #[serde(rename = "type")]
    pub chunk_type: Option<String>,
    pub rationale: Option<String>,
}

#[derive(serde::Deserialize, utoipa::ToSchema)]
pub struct UpdateChunkBody {
    pub title: Option<String>,
    pub content: Option<String>,
    #[serde(rename = "type")]
    pub chunk_type: Option<String>,
    pub rationale: Option<String>,
    pub consequences: Option<String>,
}

/// Query params arrive as strings from the web client, matching the Elysia
/// route's `t.Optional(t.String())` shape, so numeric fields parse leniently.
///
/// utoipa's `axum_extras` feature infers a handler param's location
/// (path vs query) by pattern-matching the literal identifier `Query<T>` /
/// `Path<T>` used for its extractor in the function signature. `list_chunks`
/// keeps its extractor imported under the name `Query` (it resolves to
/// `crate::extract::Query`, not `axum::extract::Query`) specifically so that
/// detection keeps working — see the comment there.
#[derive(serde::Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct ListChunksQuery {
    #[serde(rename = "type")]
    pub chunk_type: Option<String>,
    pub search: Option<String>,
    pub origin: Option<String>,
    pub review_status: Option<String>,
    pub sort: Option<Sort>,
    /// Comma-separated tag names, OR semantics — matches the `tags` key of
    /// `CollectionFilterSchema` and Node's own `?tags=` query param
    /// (`packages/api/src/chunks/service.ts:61-65`). Parsed with
    /// [`parse_tags`].
    pub tags: Option<String>,
    /// Days-ago offset (e.g. `?after=7` = "updated in the last 7 days"), not
    /// a timestamp — matches Node's `new Date(Date.now() - Number(after) *
    /// 86400000)` (`packages/api/src/chunks/service.ts:66`). Parsed with
    /// [`parse_after`].
    pub after: Option<String>,
    pub enrichment: Option<Enrichment>,
    pub min_connections: Option<String>,
    /// `None` = no space filter (every space, plus global chunks) —
    /// matches Node's `listChunks` `spaceId` branch exactly, including its
    /// "or has no space at all" half. See `chunk::ListParams::space_id`'s
    /// doc comment. Added as a side effect of threading `collection.spaceId`
    /// through `GET /collections/{id}/chunks` — a second Phase-1 parity
    /// improvement arriving alongside `tags`/`after`/`enrichment`/
    /// `minConnections`, not something this task set out to add on its own.
    pub space_id: Option<String>,
    pub limit: Option<String>,
    pub offset: Option<String>,
}

/// Splits a comma-separated tag list, trims each entry, and drops blanks —
/// matching Node's `query.tags?.split(",").map(s => s.trim()).filter(Boolean)`
/// (`packages/api/src/chunks/service.ts:61-64`). An input that is present
/// but reduces to zero non-blank entries (e.g. `""` or `","`) is treated
/// exactly like an absent `tags` param: `None`, not `Some(vec![])` — a
/// `tags = Some(vec![])` would (via `chunk::ListParams`) build an `id IN
/// (SELECT ... WHERE tag.name = ANY('{}'))` clause that matches nothing,
/// which is not what "no tag filter" means.
pub fn parse_tags(raw: &str) -> Option<Vec<String>> {
    let tags: Vec<String> = raw
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if tags.is_empty() { None } else { Some(tags) }
}

/// Converts a "days ago" string into a UTC cutoff instant — matching Node's
/// `new Date(Date.now() - Number(query.after) * 86400000)`
/// (`packages/api/src/chunks/service.ts:66`). An unparsable value is
/// treated as "no filter" (`None`) rather than reproducing JS's `Invalid
/// Date` (which Drizzle would hand to Postgres as a malformed timestamp,
/// not as "no filter") — this port has no test exercising that edge, and
/// silently ignoring an unparsable filter value is the same posture already
/// taken for `Enrichment`/`Sort`'s loose-parsing paths.
pub fn parse_after(raw: &str) -> Option<chrono::NaiveDateTime> {
    let days: i64 = raw.parse().ok()?;
    Some(chrono::Utc::now().naive_utc() - chrono::Duration::days(days))
}

impl ListChunksQuery {
    /// `limit`/`offset` are clamped here — not just inside the repository's
    /// query builder — so that whatever ends up on `params` is already the
    /// *effective* value. The route echoes `params.limit`/`params.offset`
    /// straight back in the response envelope, and that only tells the
    /// truth about pagination if it can't diverge from what the query
    /// actually used.
    pub fn into_params(self) -> fubbik_db::repo::chunk::ListParams {
        let tags = self.tags.as_deref().and_then(parse_tags);
        let after = self.after.as_deref().and_then(parse_after);
        let min_connections = self.min_connections.as_deref().and_then(|s| s.parse().ok());
        fubbik_db::repo::chunk::ListParams {
            chunk_type: self.chunk_type,
            search: self.search,
            origin: self.origin,
            review_status: self.review_status,
            sort: self.sort.unwrap_or_default(),
            tags,
            after,
            enrichment: self.enrichment,
            min_connections,
            space_id: self.space_id,
            limit: self
                .limit
                .and_then(|s| s.parse().ok())
                .unwrap_or(50)
                .clamp(1, 500),
            offset: self.offset.and_then(|s| s.parse().ok()).unwrap_or(0).max(0),
        }
    }
}

/// Response envelope for `GET /api/chunks`, matching the Node/Elysia
/// backend's `{ chunks, total, limit, offset }` shape (the web app reads
/// `.chunks` and `.total` directly). `total` is the count of rows matching
/// the same filters as `chunks` but WITHOUT `limit`/`offset` applied — see
/// `fubbik_db::repo::chunk::count`, which shares its filter-building logic
/// with `list` so the two can never disagree.
#[derive(serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChunkListResponse {
    pub chunks: Vec<Chunk>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}
