#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateTagBody {
    pub name: String,
    pub tag_type_id: Option<String>,
}

/// `tag_type_id` is deliberately `Option<Option<String>>`, not
/// `Option<String>`: it mirrors Node's `t.Optional(t.Union([t.String(),
/// t.Null()]))` body schema, which is three-state — omitted (leave
/// untouched), explicit `null` (clear), or a string (set).
///
/// Plain `#[derive(Deserialize)]` on a bare `Option<Option<String>>` field
/// does NOT give tri-state semantics: `Option<T>`'s own `Deserialize` impl
/// maps a JSON `null` straight to `None` regardless of nesting depth, so an
/// explicit `"tagTypeId": null` would be indistinguishable from the key
/// being absent entirely — both would collapse to the outer `None`. The
/// `#[serde(default, deserialize_with = "deserialize_some")]` pair below is
/// the standard fix: `default` makes a missing key `None`, while
/// `deserialize_some` runs only when the key IS present and always wraps
/// its result (even `Option::None` from a `null`) in an extra `Some`.
#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTagBody {
    pub name: Option<String>,
    #[serde(default, deserialize_with = "deserialize_some")]
    pub tag_type_id: Option<Option<String>>,
    pub review_status: Option<ReviewStatus>,
}

/// Matches Node's body schema exactly (`packages/api/src/tags/routes.ts:39`):
/// `t.Optional(t.Union([t.Literal("draft"), t.Literal("reviewed"),
/// t.Literal("approved")]))`. Modelled the same way as
/// `connections::dto::Origin` — a proper enum, not `Option<String>` — so an
/// invalid value (e.g. `"totally-bogus"`) is rejected by serde at
/// deserialisation, before it ever reaches the service or database layer.
/// There is no database check constraint backstopping this column
/// (`review_status text NOT NULL DEFAULT 'approved'`), so this enum is the
/// only thing standing between an arbitrary client string and storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReviewStatus {
    Draft,
    Reviewed,
    Approved,
}

impl ReviewStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            ReviewStatus::Draft => "draft",
            ReviewStatus::Reviewed => "reviewed",
            ReviewStatus::Approved => "approved",
        }
    }
}

fn deserialize_some<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    T: serde::Deserialize<'de>,
    D: serde::Deserializer<'de>,
{
    T::deserialize(deserializer).map(Some)
}

#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MergeBody {
    pub source_id: String,
    pub target_id: String,
}

/// Shape of every `{ message: "Deleted" }` delete response in this domain,
/// matching Node's convention (`_mutating.md`): deletes return 200, not
/// 204, with this body.
#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct MessageResponse {
    pub message: String,
}
