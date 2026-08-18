//! Request/response shapes for `packages/api/src/vocabulary/routes.ts`.

/// The six literals Elysia's `CategorySchema` accepts
/// (`packages/api/src/vocabulary/routes.ts:7-14`, a genuine `t.Union` of
/// literals — unlike `notification.type`/`connection.relation`, this *is*
/// a real input constraint). Modelled as an enum only at this input-body
/// layer, the same shape as `connections::dto::Origin`: the stored
/// `vocabulary_entry.category` column and
/// `fubbik_db::repo::vocabulary::VocabularyEntry::category` stay a plain
/// `String` end to end, since the DB has no matching check constraint and
/// Node's `SELECT *` never re-validates on read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Actor,
    Action,
    Target,
    Outcome,
    State,
    Modifier,
}

impl Category {
    pub fn as_str(self) -> &'static str {
        match self {
            Category::Actor => "actor",
            Category::Action => "action",
            Category::Target => "target",
            Category::Outcome => "outcome",
            Category::State => "state",
            Category::Modifier => "modifier",
        }
    }
}

/// `GET /api/vocabulary` query params
/// (`packages/api/src/vocabulary/routes.ts:35-37`). `spaceId` is genuinely
/// optional here — unlike every other endpoint in this domain, which 404s
/// on a missing/foreign space, the route returns `[]` outright when
/// `spaceId` is absent (`ctx.query.spaceId` falsy check,
/// `routes.ts:30`), never touching the service layer at all.
#[derive(serde::Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct ListVocabularyQuery {
    pub space_id: Option<String>,
}

/// Body of `POST /api/vocabulary/suggest` (`routes.ts:51-53`).
#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SuggestBody {
    pub space_id: String,
}

/// One entry of `POST /api/vocabulary/bulk`'s `entries` array
/// (`routes.ts:16-20`). `word` has no length cap enforced here — Node
/// declares `t.String({ maxLength: 100 })`, but (matching
/// `favorites::dto::CreateFavoriteBody`'s accepted divergence) this port
/// does not enforce it; an over-length word simply round-trips into the
/// `text` column, which has no length constraint of its own either.
#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct EntryInput {
    pub word: String,
    pub category: Category,
    pub expects: Option<Vec<String>>,
}

/// Body of `POST /api/vocabulary/bulk` (`routes.ts:69-73`).
#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BulkCreateBody {
    pub entries: Vec<EntryInput>,
    pub space_id: String,
}

/// Body of `POST /api/vocabulary/parse` (`routes.ts:86-89`). `text` has no
/// length cap enforced, same accepted divergence as `EntryInput::word`
/// (Node: `t.String({ maxLength: 1000 })`).
#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ParseBody {
    pub text: String,
    pub space_id: String,
}

/// Body of `POST /api/vocabulary` (`routes.ts:105-110`).
#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateEntryBody {
    pub word: String,
    pub category: Category,
    pub expects: Option<Vec<String>>,
    pub space_id: String,
}

/// Body of `PATCH /api/vocabulary/{id}` (`routes.ts:124-128`). All three
/// fields are two-state (`None` = leave untouched) — see
/// `fubbik_db::repo::vocabulary::VocabularyPatch`'s doc comment for why
/// there is no explicit-null-clear variant.
#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateEntryBody {
    pub word: Option<String>,
    pub category: Option<Category>,
    pub expects: Option<Vec<String>>,
}

/// Shape of `DELETE /api/vocabulary/{id}`'s `{ "message": "Deleted" }`
/// response, matching this crate's `_mutating.md` convention.
#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct MessageResponse {
    pub message: String,
}
