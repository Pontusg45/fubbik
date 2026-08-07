/// Query params arrive as strings from the web client, matching the
/// Elysia route's shape (`packages/api/src/notifications/routes.ts`):
/// `limit: t.Optional(t.Numeric())`, `unreadOnly: t.Optional(t.String())`
/// compared with `ctx.query.unreadOnly === "true"` at the route, not
/// coerced to a real boolean by the schema. `into_params`-style helpers
/// below reproduce both quirks exactly rather than using `t.Optional(t.Bool())`
/// semantics Node never had.
#[derive(serde::Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct ListNotificationsQuery {
    pub limit: Option<String>,
    pub unread_only: Option<String>,
}

impl ListNotificationsQuery {
    /// Node: `ctx.query.unreadOnly === "true"` — any other string
    /// (including `"false"`, `"1"`, or garbage) is falsy, matching exactly
    /// here.
    pub fn unread_only(&self) -> bool {
        self.unread_only.as_deref() == Some("true")
    }

    /// Node's `t.Numeric()` leaves `limit` undefined when omitted, and the
    /// repository (`listNotifications`) defaults to `50` via
    /// `opts.limit ?? 50`.
    pub fn limit(&self) -> i64 {
        self.limit
            .as_deref()
            .and_then(|s| s.parse().ok())
            .unwrap_or(50)
    }
}

/// Shape of `GET /api/notifications/count`
/// (`tests/fixtures/node-contract-2b/notifications-count.json`): a bare
/// `{ "count": N }` object, not the chunks-style envelope.
#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct CountResponse {
    pub count: i64,
}

/// Shape of every `{ message: "..." }` response in this domain, matching
/// Node's convention (`_mutating.md`) for delete (200, `"Deleted"`) and
/// `POST /notifications/read-all` (200, `"All marked as read"`).
#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct MessageResponse {
    pub message: String,
}
