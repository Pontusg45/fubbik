use fubbik_db::repo::chunk::{Chunk, Enrichment, Sort};

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct ConnectionSuggestion {
    pub id: String,
    pub title: String,
    #[serde(rename = "type")]
    pub chunk_type: String,
    pub reason: String,
}

/// Body of `POST /api/chunks`, matching Node's route schema
/// (`packages/api/src/chunks/routes.ts` — the 13-field `t.Object` on the
/// `/chunks` POST).
///
/// This used to carry four fields. Every other one the client sent was
/// **silently discarded**: serde drops unknown fields rather than
/// rejecting them, so a create with `tags`/`spaceIds`/`scope`/
/// `alternatives` returned 200 and a chunk missing all of them. That is
/// the same failure mode that made `requirements/stats` ignore `spaceId`
/// while looking healthy — it is the reason call sites must be migrated,
/// not just routes ported.
#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateChunkBody {
    pub title: String,
    #[serde(default)]
    pub content: String,
    #[serde(rename = "type")]
    pub chunk_type: Option<String>,
    /// Tag **names**, not ids — resolved through `tag::find_or_create`.
    pub tags: Option<Vec<String>>,
    pub space_ids: Option<Vec<String>>,
    #[schema(value_type = Option<std::collections::HashMap<String, String>>)]
    pub scope: Option<serde_json::Value>,
    pub rationale: Option<String>,
    pub alternatives: Option<Vec<String>>,
    pub consequences: Option<String>,
    /// `human` (the default) or `ai` — Node's route schema allows no other
    /// value. `ai` creates a `draft` chunk needing review; `human` creates
    /// an `approved` one. See `service::review_status_for_origin`.
    pub origin: Option<String>,
    pub document_id: Option<String>,
    pub document_order: Option<i32>,
    /// Free-text label recorded on the chunk's version history.
    pub update_tag: Option<String>,
}

/// Body of `PATCH /api/chunks/{id}` — same story as [`CreateChunkBody`]:
/// this carried five fields, and `tags`, `reviewStatus` and `isEntryPoint`
/// (all three sent by the chunk detail page) were being silently dropped.
///
/// `summary` is tri-state. `Option<Option<String>>` plus
/// `#[serde(default, deserialize_with = "double_option")]` distinguishes
/// "key absent" (`None`) from `"summary": null` (`Some(None)`) — see
/// `fubbik_db::repo::chunk::ChunkPatch::summary`.
#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateChunkBody {
    pub title: Option<String>,
    pub content: Option<String>,
    #[serde(rename = "type")]
    pub chunk_type: Option<String>,
    pub tags: Option<Vec<String>>,
    pub space_ids: Option<Vec<String>>,
    #[serde(default, deserialize_with = "double_option")]
    #[schema(value_type = Option<String>)]
    pub summary: Option<Option<String>>,
    pub aliases: Option<Vec<String>>,
    pub not_about: Option<Vec<String>>,
    #[schema(value_type = Option<std::collections::HashMap<String, String>>)]
    pub scope: Option<serde_json::Value>,
    pub rationale: Option<String>,
    pub alternatives: Option<Vec<String>>,
    pub consequences: Option<String>,
    pub origin: Option<String>,
    pub review_status: Option<String>,
    pub is_entry_point: Option<bool>,
    pub update_tag: Option<String>,
}

/// Deserializes an optional-and-nullable field into `Option<Option<T>>`.
///
/// serde's default handling of `Option<Option<T>>` collapses both "absent"
/// and "null" to `None`, which is exactly the distinction a tri-state PATCH
/// field needs to keep. Combined with `#[serde(default)]`, this yields
/// `None` for an absent key and `Some(None)` for an explicit `null`.
fn double_option<'de, T, D>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    T: serde::Deserialize<'de>,
    D: serde::Deserializer<'de>,
{
    serde::Deserialize::deserialize(deserializer).map(Some)
}

/// Query for `GET /api/chunks/search/semantic`
/// (`packages/api/src/chunks/routes.ts:192-206`). All values arrive as
/// strings from Node's `t.Object` schema, so `limit` is parsed rather than
/// typed — keeping the wire contract identical.
#[derive(Debug, serde::Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct SemanticSearchQuery {
    pub q: String,
    pub limit: Option<String>,
    pub exclude: Option<String>,
    pub scope: Option<String>,
}

/// Body of `POST /api/chunks/check-similar` (`chunks/routes.ts:234-239`).
#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CheckSimilarBody {
    pub title: String,
    pub content: String,
    pub exclude_id: Option<String>,
}

/// Query for `GET /api/chunks/{id}/neighbors` (`chunks/routes.ts:303`). `k`
/// arrives as a string, same reasoning as [`SemanticSearchQuery`]'s
/// `limit`.
#[derive(Debug, serde::Deserialize, utoipa::IntoParams)]
pub struct NeighborsQuery {
    pub k: Option<String>,
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
            ids: None,
            // Capped at 100, matching Node's `Math.min(Number(query.limit ?? 50), 100)`
            // (`packages/api/src/chunks/service.ts:50`) — this used to clamp to 500,
            // which was invisible to the differential harness because its only limit
            // case (`?limit=5`) sits below both caps. See
            // `chunk_list_limit_is_clamped_identically_above_both_caps` in
            // `crates/fubbik-api/tests/differential.rs` for the case that closes that
            // blind spot (divergence #11, resolved).
            limit: self
                .limit
                .and_then(|s| s.parse().ok())
                .unwrap_or(50)
                .clamp(1, 100),
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

/// Response body of `GET /api/chunks/{id}` — the *enriched* chunk detail,
/// matching Node's `getChunkDetail` return
/// (`packages/api/src/chunks/service.ts:128-186`) key for key.
///
/// Until this landed, Rust's `GET /api/chunks/{id}` returned the bare
/// [`Chunk`] row, which is why the chunk detail page, the edit page and the
/// graph side panel were all still pinned to `legacyApi`.
///
/// Two keys look redundant and are: `allDeltas` and `deltas` always hold
/// the same list. Node builds the response as `{ ...result, ..., deltas:
/// result.allDeltas }`, and `result` already carries `allDeltas`, so both
/// keys ship. Reproduced rather than trimmed — a client reading either one
/// must keep working.
#[derive(serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChunkDetail {
    /// The chunk row **after** active feature overlays have been applied.
    ///
    /// The Rust type is `serde_json::Value` because a delta is free-form
    /// JSONB and could in principle introduce a key `Chunk` does not have.
    /// It is *published* as `Chunk` anyway, deliberately: every field the
    /// feature system actually writes is a `Chunk` content field (title,
    /// content, type, rationale, alternatives, consequences, summary — see
    /// the "Features (Knowledge Overlays)" section of `CLAUDE.md`), so
    /// `Chunk` describes every response this endpoint can really produce.
    ///
    /// Publishing it as an open object instead is worse than imprecise, it
    /// is unusable: utoipa's `Object` becomes `Record<string, never>` in the
    /// generated client, under which `chunk.title` is a type error. The
    /// whole point of migrating call sites off `legacyApi` is that the
    /// generated types catch wrong field access, and a type nothing can be
    /// read from catches nothing.
    #[schema(value_type = Chunk)]
    pub chunk: serde_json::Value,
    pub connections: Vec<fubbik_db::repo::connection::ChunkConnectionDetail>,
    pub spaces: Vec<fubbik_db::repo::space::Space>,
    pub applies_to: Vec<fubbik_db::repo::chunk_meta::AppliesTo>,
    pub file_references: Vec<fubbik_db::repo::chunk_meta::FileRef>,
    pub tags: Vec<fubbik_db::repo::tag::Tag>,
    pub requirements: Vec<fubbik_db::repo::requirement::ChunkRequirement>,
    pub all_deltas: Vec<fubbik_db::repo::feature::DeltaWithFeature>,
    pub health_score: fubbik_core::health::HealthScore,
    /// Ids of the features whose deltas were actually applied to `chunk`,
    /// in application order. Underscore-prefixed on the wire because Node
    /// names it that way; `rename_all = "camelCase"` would produce
    /// `appliedFeatures` without the prefix, so both keys are spelled out.
    #[serde(rename = "_appliedFeatures")]
    pub applied_features: Vec<String>,
    /// Whether the chunk has *any* delta, active or not — the UI's "this
    /// chunk is modified in some feature" indicator. Not derivable from
    /// `_appliedFeatures`, which only counts active ones.
    #[serde(rename = "_hasDeltas")]
    pub has_deltas: bool,
    pub deltas: Vec<fubbik_db::repo::feature::DeltaWithFeature>,
}
