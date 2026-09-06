use fubbik_db::repo::proposal::ProposedChanges;

/// Body of `POST /chunks/{id}/proposals`
/// (`packages/api/src/proposals/routes.ts:24-39`). `changes` is required as
/// an object but every one of its eight keys is itself optional — see
/// `ProposedChanges`'s doc comment. The service rejects an all-omitted
/// `changes` (`{}`) with 400, matching Node's
/// `Object.keys(body.changes).length === 0` check
/// (`packages/api/src/proposals/service.ts:17-18`).
#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateProposalBody {
    pub changes: ProposedChanges,
    pub reason: Option<String>,
}

/// Query params of `GET /chunks/{id}/proposals`
/// (`packages/api/src/proposals/routes.ts:50-54`). `status` is forwarded
/// unvalidated — see `fubbik_db::repo::proposal::list_for_chunk`'s doc
/// comment on why this endpoint's validation posture deliberately differs
/// from the global queue's.
#[derive(serde::Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct ListChunkProposalsQuery {
    pub status: Option<String>,
}

/// Query params of `GET /proposals`, the global queue
/// (`packages/api/src/proposals/routes.ts:102-109`). `status`, when
/// present, is validated against `pending`/`approved`/`rejected` at the
/// service layer (400 otherwise, matching Node's
/// `packages/api/src/proposals/service.ts:39-41`); when absent it defaults
/// to `"pending"` rather than "every status" — see
/// `fubbik_api::proposals::service::list`'s doc comment.
#[derive(serde::Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct ListProposalsQuery {
    pub status: Option<String>,
    pub chunk_id: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// Body of `POST /proposals/{id}/approve` and `POST /proposals/{id}/reject`
/// (`packages/api/src/proposals/routes.ts:126-129,142-144`) — identical
/// shape for both.
#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReviewBody {
    pub note: Option<String>,
}

/// `t.Union([t.Literal("approve"), t.Literal("reject")])`
/// (`packages/api/src/proposals/routes.ts:78`) — the one closed set in this
/// domain's wire shapes, unlike `status` (see `ProposedChanges`'s doc
/// comment on why that one stays a plain unvalidated-at-the-DTO-layer
/// string). Lowercase to match the literal strings Node's schema accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum BulkAction {
    Approve,
    Reject,
}

/// One entry of `POST /proposals/bulk`'s `actions` array
/// (`packages/api/src/proposals/routes.ts:75-81`).
#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BulkActionItem {
    pub proposal_id: String,
    pub action: BulkAction,
    pub note: Option<String>,
}

/// Body of `POST /proposals/bulk`. Rust applies the batch in one transaction;
/// any invalid or unauthorized item rolls every earlier action back.
#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
// Distinct OpenAPI name: `requirements::dto::BulkActionBody` is a different
// shape ({action, ids} vs {actions: [{proposalId, action}]}) and both would
// otherwise register as `#/components/schemas/BulkActionBody`, publishing
// whichever won and silently rejecting valid calls to the other endpoint.
#[schema(as = ProposalBulkActionBody)]
pub struct BulkActionBody {
    pub actions: Vec<BulkActionItem>,
}

/// Shape of `GET /proposals/count`
/// (`packages/api/src/proposals/routes.ts:57-64`): `{ pending: N }` — Node's
/// route does `Effect.map(pending => ({ pending }))` explicitly, not a bare
/// number and not `{ count: N }` like the sibling `notifications` count
/// endpoint. The web dashboard's `stats-bar.tsx` reads `.pending` off this
/// response — the whole reason this domain is on the phase-2e critical
/// path, see this crate's proposals module doc comment.
#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct PendingCountResponse {
    pub pending: i64,
}
