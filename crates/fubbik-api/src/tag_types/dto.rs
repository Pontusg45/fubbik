#[derive(serde::Deserialize, utoipa::ToSchema)]
pub struct CreateTagTypeBody {
    pub name: String,
    pub color: Option<String>,
    pub icon: Option<String>,
}

/// `icon` is deliberately `Option<Option<String>>`, not `Option<String>`: it
/// mirrors Node's `t.Optional(t.Union([t.String(), t.Null()]))` body schema
/// for `icon`, which is three-state — omitted (leave untouched), explicit
/// `null` (clear), or a string (set). `color` stays plain `Option<String>`
/// because Node's schema for it has no `t.Null()` union
/// (`t.Optional(t.String({ maxLength: 7 }))`) — it is not clearable via this
/// endpoint.
///
/// Plain `#[derive(Deserialize)]` on a bare `Option<Option<String>>` field
/// does NOT give tri-state semantics: `Option<T>`'s own `Deserialize` impl
/// maps a JSON `null` straight to `None` regardless of nesting depth, so an
/// explicit `"icon": null` would be indistinguishable from the key being
/// absent entirely — both would collapse to the outer `None`. The
/// `#[serde(default, deserialize_with = "deserialize_some")]` pair below is
/// the same fix used by `tags::dto::UpdateTagBody::tag_type_id` and
/// `spaces::dto::UpdateSpaceBody::{description, remote_url}`: `default`
/// makes a missing key `None`, while `deserialize_some` runs only when the
/// key IS present and always wraps its result (even `Option::None` from a
/// `null`) in an extra `Some`.
#[derive(serde::Deserialize, utoipa::ToSchema)]
pub struct UpdateTagTypeBody {
    pub name: Option<String>,
    pub color: Option<String>,
    #[serde(default, deserialize_with = "deserialize_some")]
    pub icon: Option<Option<String>>,
}

fn deserialize_some<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    T: serde::Deserialize<'de>,
    D: serde::Deserializer<'de>,
{
    T::deserialize(deserializer).map(Some)
}

/// Shape of every `{ message: "Deleted" }` delete response in this domain,
/// matching Node's convention (`_mutating.md`): deletes return 200, not
/// 204, with this body.
#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct MessageResponse {
    pub message: String,
}
