/// Query params for `GET /api/activity`, matching Node's Elysia schema
/// (`packages/api/src/activity/routes.ts:23-28`): all four fields are
/// optional, `limit`/`offset` arrive as strings (`t.Numeric()` from the web
/// client, parsed here the same way `chunks::dto::ListChunksQuery` does).
/// `entityId` is deliberately absent — the service/repo layers both accept
/// it, but no route ever wires it up in Node (`_questions.md` Q4), so
/// there is nothing to port at the HTTP boundary.
#[derive(serde::Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct ListActivityQuery {
    pub space_id: Option<String>,
    pub entity_type: Option<String>,
    pub limit: Option<String>,
    pub offset: Option<String>,
}

impl ListActivityQuery {
    /// Mirrors Node's `listActivityRepo` defaults exactly (`opts.limit ??
    /// 50`, `opts.offset ?? 0`) with no additional clamping — Node never
    /// clamps these either.
    pub fn into_params(self) -> fubbik_db::repo::activity::ListParams {
        fubbik_db::repo::activity::ListParams {
            space_id: self.space_id,
            entity_type: self.entity_type,
            limit: self.limit.and_then(|s| s.parse().ok()).unwrap_or(50),
            offset: self.offset.and_then(|s| s.parse().ok()).unwrap_or(0),
        }
    }
}
