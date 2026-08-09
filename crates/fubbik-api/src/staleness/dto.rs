use fubbik_db::repo::staleness::ListParams;

/// Query params for `GET /api/chunks/stale`
/// (`packages/api/src/staleness/routes.ts`'s query schema: `reason`,
/// `spaceId` as plain optional strings, `limit` as `t.Optional(t.Numeric())`).
/// `limit` arrives as a string from the query string, same pattern as
/// `notifications::dto::ListNotificationsQuery`.
#[derive(serde::Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct ListStaleQuery {
    pub reason: Option<String>,
    pub space_id: Option<String>,
    pub limit: Option<String>,
}

impl ListStaleQuery {
    pub fn into_params(self) -> ListParams {
        ListParams {
            reason: self.reason,
            space_id: self.space_id,
            limit: self.limit.as_deref().and_then(|s| s.parse().ok()),
        }
    }
}

/// Query params for `GET /api/chunks/stale/count` — `spaceId` only, no
/// `reason` (`packages/api/src/staleness/routes.ts`'s count query schema).
#[derive(serde::Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct CountQuery {
    pub space_id: Option<String>,
}

/// Body of `POST /api/chunks/suppress-duplicate`
/// (`packages/api/src/staleness/routes.ts:63-66`).
#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SuppressDuplicateBody {
    pub chunk_id_a: String,
    pub chunk_id_b: String,
}

/// Body of `POST /api/chunks/stale/scan-age`
/// (`packages/api/src/staleness/routes.ts:87-90`). Both fields optional:
/// an absent `thresholdDays` lets `detect_age_stale_chunks` fall back to
/// its own default (90); `detect_uncovered_chunks`'s threshold (30) is
/// never overridable from this body at all — see the doc comment on
/// `service::scan_age`.
#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ScanAgeBody {
    pub space_id: Option<String>,
    pub threshold_days: Option<i64>,
}

/// Shape of `POST /api/chunks/stale/scan-age`'s response: the *sum* of
/// both detectors' newly-flagged counts, matching Node's
/// `{ flagged: ageResult.flagged + uncoveredResult.flagged }`.
///
/// Also reused as `POST /api/chunks/{id}/scan-impact`'s response shape:
/// Node's `flagImpactRipple` returns the same `{ flagged: n }` envelope
/// (`packages/api/src/staleness/detect-impact.ts:8,30`).
#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct ScanResult {
    pub flagged: i64,
}

/// Body of `POST /api/chunks/{id}/scan-impact`
/// (`packages/api/src/staleness/routes.ts:101-104`). `title` is optional —
/// an absent value falls back to `"Unknown"` in the detail message Node
/// writes (`ctx.body.title ?? "Unknown"`, `routes.ts:98`), reproduced the
/// same way at the service call site.
#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ScanImpactBody {
    pub title: Option<String>,
}
