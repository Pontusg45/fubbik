use std::collections::HashMap;

/// Bare `{key: value}` map — the response shape of every "get all settings
/// for a scope" endpoint (`GET /api/settings/user`, `/codebase`,
/// `/instance`), matching Node's `Record<string, unknown>` reduction
/// (`packages/api/src/settings/service.ts`). Not an envelope, not an
/// array — see `tests/fixtures/node-contract-2b/_questions.md` Q3:
/// `settings-user.json` / `settings-codebase.json` / `settings-instance.json`
/// are all bare objects (`{}` in the captured, empty-table fixtures).
///
/// Used as the actual Rust return type end to end
/// (`service::get_all_user_settings` et al. build and return one). The
/// `#[utoipa::path]` annotations on the three `GET` handlers that return
/// this document `body = serde_json::Value` instead, purely because
/// `utoipa` 5's blanket `HashMap<K, V>` schema impl requires `V:
/// ComposeSchema`, which `serde_json::Value` (deliberately arbitrary here)
/// doesn't implement — `serde_json::Value` itself does implement
/// `ToSchema`, so documenting the response as "an arbitrary JSON value"
/// is both accurate (every value in this map really can be anything) and
/// the only option that compiles. This does not affect the wire format;
/// `axum::Json<SettingsMap>` still serialises as a plain `{key: value}`
/// object.
pub type SettingsMap = HashMap<String, serde_json::Value>;

/// Body of `PATCH /api/settings/user` and `PATCH /api/settings/instance`
/// (`packages/api/src/settings/routes.ts`): both share the identical
/// `{ key, value }` shape. `value` is a bare `serde_json::Value`, not a
/// typed union — Node's schema is `value: t.Unknown()`, i.e. **no
/// validation at all** on the value's shape, despite the typed
/// `UserSettingsMap` / `InstanceSettingsMap` interfaces in
/// `packages/db/src/schema/settings.ts:54-80` existing purely as TS-side
/// documentation, never enforced at the API boundary. A client can PATCH
/// `theme` to `42` and this stores it as-is, matching Node exactly.
#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SetSettingBody {
    pub key: String,
    pub value: serde_json::Value,
}

/// Body of `PATCH /api/settings/codebase`
/// (`packages/api/src/settings/routes.ts:38-54`). `codebaseId` here is a
/// `spaceId` — the field name is a legacy holdover predating the
/// codebase->space rename elsewhere in the codebase (see the top-level
/// CLAUDE.md's "Spaces & Workspaces" section and `codebase_settings`'s
/// own doc comment in `fubbik_db::repo::settings`).
#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SetCodebaseSettingBody {
    pub codebase_id: String,
    pub key: String,
    pub value: serde_json::Value,
}

/// Query of `GET /api/settings/codebase?codebaseId=...`.
#[derive(serde::Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct CodebaseQuery {
    pub codebase_id: String,
}

/// Shape of every settings `PATCH` response — `{ "message": "Updated" }`,
/// status 200, on all three scopes (`packages/api/src/settings/routes.ts`).
/// Distinct wording from the `{ "message": "Deleted" }` convention used by
/// delete endpoints elsewhere in this crate.
#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct MessageResponse {
    pub message: String,
}

/// `GET /api/settings/features` — a **computed 5-key projection** over
/// `instance_settings`, not a stored table
/// (`tests/fixtures/node-contract-2b/_questions.md` Q2). Every field
/// independently defaults to `true` when its key is absent from
/// `instance_settings`, matching Node's `(map.x as boolean) ?? true`
/// fallback (`packages/api/src/settings/service.ts:59-75`) for the
/// documented case — a key that is simply absent. Confirmed live against
/// the empty-table fixture: `settings-features.json` returns all five
/// `true` even though `instance_settings` has zero rows.
///
/// Node's `?? true` only falls back on `null`/`undefined`, and its `as
/// boolean` cast is a compile-time-only no-op — so a value present but not
/// actually boolean (e.g. a stored string) would pass through unchanged at
/// runtime, breaking `InstanceSettingsMap`'s own typing. Rust's `bool`
/// fields can't reproduce that untyped leak; `service::get_feature_flags`
/// treats "present but not a JSON boolean" the same as "absent" (defaults
/// `true`), which is a consequence of Rust's stricter typing, not an
/// untested behavioural choice — the only case the fixtures actually
/// exercise is "absent", and that path matches Node exactly.
#[derive(serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FeatureFlags {
    pub ai_enabled: bool,
    pub enrichment_enabled: bool,
    pub semantic_search_enabled: bool,
    pub ai_suggestions_enabled: bool,
    pub vocabulary_suggest_enabled: bool,
}
