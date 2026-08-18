use fubbik_db::repo::template::{FieldMapping, MatchRules};

/// Body of `POST /api/templates` (`packages/api/src/templates/routes.ts:48-73`).
///
/// `description`/`matchRules`/`fieldMappings` are all `t.Optional(t.Union([
/// Schema, t.Null()]))` in Node, but on *create* there's no existing value
/// to distinguish "leave untouched" from — Node's `createTemplateRepo`
/// collapses omitted and explicit `null` to the same stored `NULL`
/// (`packages/db/src/repository/template.ts:42,46-47`) — so plain
/// `Option<T>` (not the tri-state `Option<Option<T>>` `UpdateTemplateBody`
/// needs) is correct here.
///
/// `name`/`description`/`type`/`content`/tag length caps (`maxLength: 100/
/// 500/20/50000/50`, `tags` `maxItems: 20`) are Elysia-only request
/// validation in Node; matching `favorites::dto::CreateFavoriteBody`'s
/// precedent, this port does not enforce them — an over-long value simply
/// gets stored as-is instead of being rejected up front.
///
/// `type` is free text with only a length cap in Node (no enum, no CHECK
/// constraint) despite looking like it should be one of the five chunk
/// kinds — see `fubbik_db::repo::template::Template`'s doc comment.
#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateTemplateBody {
    pub name: String,
    pub description: Option<String>,
    #[serde(rename = "type")]
    pub template_type: String,
    pub content: String,
    pub match_rules: Option<MatchRules>,
    pub field_mappings: Option<Vec<FieldMapping>>,
    pub priority: Option<i32>,
    pub tags: Option<Vec<String>>,
}

/// Body of `PATCH /api/templates/{id}` (`packages/api/src/templates/routes.ts:74-94`).
///
/// `description`/`matchRules`/`fieldMappings` are tri-state: omitted
/// (`None`) leaves the column untouched, explicit `null` (`Some(None)`)
/// clears it, a value (`Some(Some(..))`) sets it — matching Node's
/// `params.field !== undefined` conditional spread
/// (`packages/db/src/repository/template.ts:74-83`), which forwards an
/// explicit `null` through as a real update. Same `deserialize_some` trick
/// as `workspaces::dto::UpdateWorkspaceBody::description`.
///
/// `priority`/`tags` have no `t.Null()` variant in Node's schema, so they
/// stay plain `Option<T>` — omitted leaves them untouched, there is no way
/// to explicitly clear either through this endpoint.
#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTemplateBody {
    pub name: Option<String>,
    #[serde(default, deserialize_with = "deserialize_some")]
    pub description: Option<Option<String>>,
    #[serde(rename = "type")]
    pub template_type: Option<String>,
    pub content: Option<String>,
    #[serde(default, deserialize_with = "deserialize_some")]
    pub match_rules: Option<Option<MatchRules>>,
    #[serde(default, deserialize_with = "deserialize_some")]
    pub field_mappings: Option<Option<Vec<FieldMapping>>>,
    pub priority: Option<i32>,
    pub tags: Option<Vec<String>>,
}

fn deserialize_some<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    T: serde::Deserialize<'de>,
    D: serde::Deserializer<'de>,
{
    T::deserialize(deserializer).map(Some)
}

/// Shape of the `{ message: "Deleted" }` response, matching every other
/// domain's delete convention (`_mutating.md`).
#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct MessageResponse {
    pub message: String,
}
