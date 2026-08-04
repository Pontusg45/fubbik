#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateSpaceBody {
    pub name: String,
    pub kind: Option<String>,
    pub description: Option<String>,
    pub remote_url: Option<String>,
    pub local_paths: Option<Vec<String>>,
}

/// `description` and `remote_url` are tri-state, matching Node's
/// `t.Optional(t.Union([t.String(), t.Null()]))` for both: omitted leaves
/// the field untouched (or, for `remoteUrl` on a code-kind space, falls
/// through to Node's `?? null` — see `service::update`), explicit `null`
/// clears it, a string sets it. Same `deserialize_some` trick as
/// `tags::dto::UpdateTagBody::tag_type_id` — plain `Option<Option<T>>`
/// collapses "omitted" and "explicit null" to the same `None` without it.
///
/// `local_paths` is NOT tri-state at the schema level (`t.Optional(t.Array(...))`,
/// no `t.Null()` union) — a plain `Option<Vec<String>>` is correct here.
#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSpaceBody {
    pub name: Option<String>,
    #[serde(default, deserialize_with = "deserialize_some")]
    pub description: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_some")]
    pub remote_url: Option<Option<String>>,
    pub local_paths: Option<Vec<String>>,
}

fn deserialize_some<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    T: serde::Deserialize<'de>,
    D: serde::Deserializer<'de>,
{
    T::deserialize(deserializer).map(Some)
}

#[derive(serde::Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct DetectQuery {
    pub remote_url: Option<String>,
    pub local_path: Option<String>,
}

/// Shape of every `{ message: "Deleted" }` delete response in this domain,
/// matching Node's convention (`_mutating.md`): deletes return 200, not
/// 204, with this body.
#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct MessageResponse {
    pub message: String,
}
