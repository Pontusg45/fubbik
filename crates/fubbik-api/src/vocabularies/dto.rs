//! Request/response shapes for `packages/api/src/vocabularies/routes.ts`.
//!
//! Every type here carries a domain-specific name (`...ChunkType...` /
//! `...ConnectionRelation...`) rather than a bare `CreateBody`/`Query`,
//! because utoipa registers `ToSchema` types in one flat namespace and the
//! two catalogs ported here have near-identical shapes — a generic name
//! would collide silently. See `tests/schema_names.rs`.

/// `arrowStyle` on the connection-relation bodies — a genuine Elysia
/// `t.Union` of literals (`packages/api/src/vocabularies/routes.ts:30,40`),
/// so an enum here matches the contract rather than inventing a constraint.
///
/// Modelled *only* on the input side: the stored `arrow_style` column is
/// plain `text` with no CHECK constraint, and
/// `fubbik_db::repo::connection_relation::ConnectionRelation::arrow_style`
/// stays a `String` on the read path — the same split
/// `vocabulary::dto::Category` makes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum ArrowStyle {
    Solid,
    Dashed,
    Dotted,
}

impl ArrowStyle {
    pub fn as_str(self) -> &'static str {
        match self {
            ArrowStyle::Solid => "solid",
            ArrowStyle::Dashed => "dashed",
            ArrowStyle::Dotted => "dotted",
        }
    }
}

/// `direction` on the connection-relation bodies — likewise a real
/// `t.Union` of literals (`routes.ts:31,41`).
///
/// Named `RelationDirection`, not `Direction`, to keep the flat OpenAPI
/// schema namespace unambiguous.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum RelationDirection {
    Forward,
    Bidirectional,
}

impl RelationDirection {
    pub fn as_str(self) -> &'static str {
        match self {
            RelationDirection::Forward => "forward",
            RelationDirection::Bidirectional => "bidirectional",
        }
    }
}

/// `GET /api/chunk-types` query params
/// (`packages/api/src/vocabularies/routes.ts:56`).
///
/// `rename_all = "camelCase"` is mandatory, not cosmetic: the wire name is
/// `spaceId`, and serde would silently *ignore* a `spaceId` key on a struct
/// with a `space_id` field, leaving the parameter permanently unset while
/// the endpoint looked healthy.
#[derive(serde::Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct ChunkTypesQuery {
    pub space_id: Option<String>,
}

/// `GET /api/connection-relations` query params (`routes.ts:99`). A
/// separate type from `ChunkTypesQuery` despite the identical shape, so
/// each endpoint's generated parameter documentation stays independent.
#[derive(serde::Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionRelationsQuery {
    pub space_id: Option<String>,
}

/// Body of `POST /api/chunk-types`
/// (`packages/api/src/vocabularies/routes.ts:7-15,58-72`).
///
/// `id` is caller-supplied — it is the table's primary key and the slug
/// `chunk.type` will hold. Unlike the Elysia `maxLength` caps elsewhere in
/// this port (which are deliberately not enforced), `id`'s length *is*
/// enforced, because Node validates it in the **service** layer with
/// `SLUG_RE = /^[a-z0-9][a-z0-9_-]{0,40}$/`
/// (`packages/api/src/vocabularies/service.ts:38,42-46`) — a real 400, not
/// just an Elysia schema check. See `service::validate_slug`.
///
/// `description`/`icon` are `t.Optional(t.Union([t.String, t.Null()]))` in
/// Node, but on *create* there is no prior value to distinguish "omitted"
/// from "explicitly null" — Node's `createChunkType` collapses both to a
/// stored `NULL` (`packages/db/src/repository/vocabulary-catalog.ts:63-64`)
/// — so plain `Option<T>` is correct here, unlike the update body.
///
/// `label`/`description`/`icon`/`color`/`examples` length and item caps are
/// Elysia-only request validation; matching this port's established
/// precedent (`templates::dto::CreateTemplateBody`), they are not enforced.
#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateChunkTypeBody {
    pub id: String,
    pub label: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub examples: Option<Vec<String>>,
    pub display_order: Option<i32>,
}

/// Body of `PATCH /api/chunk-types/{id}` (`routes.ts:17-24,73-82`).
///
/// `description`/`icon` are tri-state: omitted (`None`) leaves the column
/// untouched, explicit `null` (`Some(None)`) clears it, a value sets it —
/// matching Node's `data.field !== undefined` conditional spread
/// (`packages/db/src/repository/vocabulary-catalog.ts:88-94`), which
/// forwards an explicit `null` through as a real update. `color`,
/// `examples` and `displayOrder` have no `t.Null()` variant in Node's PATCH
/// schema, so they stay plain `Option<T>` — there is no way to clear them.
///
/// `id` is deliberately absent: Node's patch schema omits it, so a chunk
/// type's slug can never be renamed through this endpoint.
#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateChunkTypeBody {
    pub label: Option<String>,
    #[serde(default, deserialize_with = "deserialize_some")]
    pub description: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_some")]
    pub icon: Option<Option<String>>,
    pub color: Option<String>,
    pub examples: Option<Vec<String>>,
    pub display_order: Option<i32>,
}

/// Body of `POST /api/connection-relations` (`routes.ts:26-35,101-115`).
///
/// Same create-time collapse of `null` and omitted as
/// `CreateChunkTypeBody`; `inverseOfId` is a self-FK to
/// `connection_relation(id)` (`ON DELETE SET NULL`), so an unknown value
/// surfaces as a database error — Node behaves identically, it does no
/// pre-check either.
#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateConnectionRelationBody {
    pub id: String,
    pub label: String,
    pub description: Option<String>,
    pub arrow_style: Option<ArrowStyle>,
    pub direction: Option<RelationDirection>,
    pub color: Option<String>,
    pub inverse_of_id: Option<String>,
    pub display_order: Option<i32>,
}

/// Body of `PATCH /api/connection-relations/{id}` (`routes.ts:37-45,116-125`).
///
/// `description` and `inverseOfId` are the two tri-state fields (both have
/// a `t.Null()` variant in Node's schema); everything else is plain
/// `Option<T>`.
#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateConnectionRelationBody {
    pub label: Option<String>,
    #[serde(default, deserialize_with = "deserialize_some")]
    pub description: Option<Option<String>>,
    pub arrow_style: Option<ArrowStyle>,
    pub direction: Option<RelationDirection>,
    pub color: Option<String>,
    #[serde(default, deserialize_with = "deserialize_some")]
    pub inverse_of_id: Option<Option<String>>,
    pub display_order: Option<i32>,
}

/// Distinguishes "key absent" from "key present with value `null`", which
/// plain `Option<Option<T>>` cannot — same helper as
/// `templates::dto`/`workspaces::dto`.
fn deserialize_some<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    T: serde::Deserialize<'de>,
    D: serde::Deserializer<'de>,
{
    T::deserialize(deserializer).map(Some)
}

/// Shape of the `{ message: "Deleted" }` response both DELETEs return
/// (`routes.ts:87,130`), matching every other domain's delete convention.
#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct MessageResponse {
    pub message: String,
}
