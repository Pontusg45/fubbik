use fubbik_db::repo::coverage::CoverageMatrixRow;

/// Query params of `GET /requirements/coverage`
/// (`packages/api/src/coverage/routes.ts:20-25`).
///
/// **`codebaseId`, not `spaceId`.** `codebase` is the deprecated name for
/// `space` everywhere else in this codebase, but this endpoint's wire
/// contract still spells it `codebaseId` and both web call sites send it
/// that way (`apps/web/src/routes/coverage.tsx:101-103`). Renaming it here
/// would silently drop the filter for every existing client, so it stays.
///
/// `detail` is `t.Optional(t.String())` — a **string**, not a boolean, and
/// the route compares it with `ctx.query.detail === "true"`
/// (`packages/api/src/coverage/routes.ts:14`). `?detail=1`, `?detail=TRUE`
/// and `?detail=yes` therefore all take the *default* branch in Node.
/// Modelling it as `Option<bool>` would accept `?detail=1` and change
/// behaviour, so it stays a `String` compared literally — see
/// `service::wants_detail`.
#[derive(serde::Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct CoverageQuery {
    pub codebase_id: Option<String>,
    pub detail: Option<String>,
}

/// Query params of `GET /requirements/traceability`
/// (`packages/api/src/coverage/routes.ts:33-37`). Same `codebaseId`
/// spelling as `CoverageQuery`, and no `detail` — traceability has only one
/// shape.
#[derive(serde::Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct TraceabilityQuery {
    pub codebase_id: Option<String>,
}

/// A chunk with at least one requirement pointing at it
/// (`packages/api/src/coverage/service.ts:23,29`). Carries the count;
/// `UncoveredChunk` deliberately does not, because Node's uncovered entries
/// are `{id, title}` only — the count there is always `0` and Node omits it.
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CoveredChunk {
    pub id: String,
    pub title: String,
    pub requirement_count: i64,
}

/// A chunk no requirement references
/// (`packages/api/src/coverage/service.ts:24,31`). Two fields, no
/// `requirementCount` — see `CoveredChunk`.
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UncoveredChunk {
    pub id: String,
    pub title: String,
}

/// `packages/api/src/coverage/service.ts:43`. All four are plain numbers on
/// the wire.
///
/// `percentage` is `Math.round((covered / total) * 100)` guarded by
/// `total > 0 ? ... : 0` — the guard matters because in JS `0 / 0` is `NaN`
/// and `Math.round(NaN)` is `NaN`, which `JSON.stringify` emits as `null`.
/// Rust would produce `NaN` too and then fail to serialise it, so the same
/// zero-total guard is reproduced in `service::get_coverage`.
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CoverageStats {
    pub total: i64,
    pub covered: i64,
    pub uncovered: i64,
    pub percentage: i64,
}

/// Body of `GET /api/requirements/coverage`.
///
/// **This one struct is two response shapes.** Node's route dispatches on
/// `detail` to two different service functions
/// (`packages/api/src/coverage/routes.ts:14-17`): `getCoverage` returns
/// `{covered, uncovered, stats}`, and `getCoverageMatrix` returns that
/// object spread with an extra `matrix` key
/// (`packages/api/src/coverage/service.ts:9-12`). So `matrix` is *absent*
/// from the default response, not `null` and not `[]` — hence
/// `skip_serializing_if`, which drops the key entirely when `None`.
///
/// The distinction is load-bearing rather than cosmetic: computing `matrix`
/// is a second query over `requirement_chunk`, and the default path must not
/// run it. That is why this is an `Option` populated by one of two service
/// functions instead of a field that is always filled in.
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CoverageResponse {
    pub covered: Vec<CoveredChunk>,
    pub uncovered: Vec<UncoveredChunk>,
    pub stats: CoverageStats,
    /// Present only for `?detail=true`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matrix: Option<Vec<CoverageMatrixRow>>,
}

/// One row of `GET /api/requirements/traceability`. The endpoint returns a
/// **bare array** of these, not an envelope
/// (`packages/api/src/coverage/service.ts:16-18` returns the repository
/// result directly).
///
/// `planSteps` and `sessions` are **always empty**. Node hard-codes them:
/// `requirements.map(req => ({...req, planSteps: [] as unknown[], sessions:
/// [] as unknown[]}))` (`packages/db/src/repository/coverage.ts:93-97`),
/// under a `TODO` saying traceability needs reworking now that the plans
/// rewrite deleted `implementationSession`, `sessionRequirementRef` and
/// `planStep`. The fields are kept, still empty, because
/// `apps/web/src/features/coverage/traceability-content.tsx:52-54` reads
/// `.length` on both and would crash on `undefined`. `serde_json::Value`
/// element type mirrors Node's `unknown[]`: nothing populates them, so
/// there is no element shape to name.
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TraceabilityRow {
    pub id: String,
    pub title: String,
    pub status: String,
    pub priority: Option<String>,
    #[schema(value_type = Vec<serde_json::Value>)]
    pub plan_steps: Vec<serde_json::Value>,
    #[schema(value_type = Vec<serde_json::Value>)]
    pub sessions: Vec<serde_json::Value>,
}
