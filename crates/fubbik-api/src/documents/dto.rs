use fubbik_db::repo::chunk::Chunk;
use fubbik_db::repo::document::Document;
use fubbik_db::timestamp::UtcTimestamp;

/// `GET /api/documents` query, matching `packages/api/src/documents/routes.ts:16-20`.
#[derive(serde::Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct ListDocumentsQuery {
    pub space_id: Option<String>,
}

/// `GET /api/documents/search` query, matching
/// `packages/api/src/documents/routes.ts:30-35`. `q`'s `minLength: 2` is
/// enforced in `routes::search_documents` (see that function's doc
/// comment for why, unlike this port's usual "Elysia-only caps go
/// unenforced" precedent).
#[derive(serde::Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct SearchDocumentsQuery {
    pub q: String,
    pub space_id: Option<String>,
}

/// Body of `POST /api/documents/import`
/// (`packages/api/src/documents/routes.ts:57-63`). `sourcePath`/`content`
/// length caps (`maxLength: 500`/`200000`) are Elysia-only request
/// validation Node never re-checks past the transport layer; matching
/// this port's established precedent (see
/// `crate::templates::dto::CreateTemplateBody`'s doc comment), they go
/// unenforced here — an over-long value is simply stored as-is.
///
/// Node's route never accepts or forwards a `templateId`
/// (`packages/api/src/documents/service.ts`'s `importDocument` takes one,
/// but no caller — HTTP or otherwise — ever supplies it), so this body has
/// no such field either; see `service::import_document`'s doc comment.
#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ImportDocumentBody {
    pub source_path: String,
    pub content: String,
    pub space_id: Option<String>,
}

/// One entry of `POST /api/documents/import-dir`'s `files` array
/// (`packages/api/src/documents/routes.ts:78-85`).
#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ImportFileEntry {
    pub source_path: String,
    pub content: String,
}

/// Body of `POST /api/documents/import-dir`. `files`' `maxItems: 200` is
/// likewise unenforced — see [`ImportDocumentBody`]'s doc comment.
#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ImportDirBody {
    pub files: Vec<ImportFileEntry>,
    pub space_id: Option<String>,
}

/// Body of `POST /api/documents/{id}/sync`
/// (`packages/api/src/documents/routes.ts:100-106`).
#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SyncDocumentBody {
    pub content: String,
    pub space_id: Option<String>,
}

/// `"unchanged" | "created" | "synced"` — the three literal values
/// `importDocument`/`syncDocument` actually return
/// (`packages/api/src/documents/service.ts:53,59,114,157,168,237`). There
/// is no `"updated"` variant: a re-sync with content changes reports
/// `status: "synced"` regardless of whether any section was individually
/// created vs. updated (see `created`/`updated` counts for that detail).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum ImportStatus {
    Unchanged,
    Created,
    Synced,
}

/// Shape of `POST /api/documents/import` and each element of
/// `POST /api/documents/import-dir`'s response array, matching Node's
/// `importDocument` return object exactly
/// (`packages/api/src/documents/service.ts:45-158`).
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub document: Document,
    pub created: i32,
    pub updated: i32,
    pub status: ImportStatus,
    pub first_chunk_id: Option<String>,
}

/// Shape of `POST /api/documents/{id}/sync`, matching Node's
/// `syncDocument` return object (`packages/api/src/documents/service.ts:161-239`).
/// **`document` is the pre-sync snapshot, not the freshly-updated row** —
/// see `service::sync_document`'s doc comment; this is Node's actual
/// return expression (`doc` is never reassigned after the `updateDocument`
/// call), reproduced deliberately rather than "fixed".
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SyncResult {
    pub document: Document,
    pub created: i32,
    pub updated: i32,
    pub status: ImportStatus,
}

/// Shape of `GET /api/documents/{id}/render`, matching Node's
/// `renderDocument` return object (`packages/api/src/documents/service.ts:241-278`):
/// `{ document, markdown }`, nested — **not** flattened, unlike
/// [`DocumentDetail`].
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RenderResult {
    pub document: Document,
    pub markdown: String,
}

/// Shape of `GET /api/documents/{id}`, matching Node's `getDocument`
/// return expression: `{ ...doc, chunks }`
/// (`packages/api/src/documents/service.ts:288-295`) — a genuine object
/// spread, so the document's own fields sit at the *top level* alongside
/// `chunks`, not nested under a `document` key. Modelled as an explicit
/// flat struct (rather than `#[serde(flatten)]` on a nested `Document`)
/// since no other DTO in this crate uses `flatten` and this keeps the
/// utoipa-generated schema unambiguous.
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DocumentDetail {
    pub id: String,
    pub title: String,
    pub source_path: String,
    pub content_hash: String,
    pub description: Option<String>,
    pub split_level: Option<i32>,
    pub space_id: Option<String>,
    pub user_id: String,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub created_at: UtcTimestamp,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub updated_at: UtcTimestamp,
    pub chunks: Vec<Chunk>,
}

impl DocumentDetail {
    pub fn new(doc: Document, chunks: Vec<Chunk>) -> Self {
        Self {
            id: doc.id,
            title: doc.title,
            source_path: doc.source_path,
            content_hash: doc.content_hash,
            description: doc.description,
            split_level: doc.split_level,
            space_id: doc.space_id,
            user_id: doc.user_id,
            created_at: doc.created_at,
            updated_at: doc.updated_at,
            chunks,
        }
    }
}
