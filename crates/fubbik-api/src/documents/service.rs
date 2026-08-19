//! Business logic for the `documents` domain. See `fubbik_db::repo::document`'s
//! module doc comment for how a document relates to chunks (a plain
//! `chunk.document_id`/`chunk.document_order` pair, owned by this domain —
//! not a join table), and for the intentionally-reproduced `document_order`
//! staleness bug this module's `sync_document` triggers via
//! `document::touch_chunk`.

use fubbik_core::error::{AppError, AppResult};
use fubbik_db::repo::chunk::Chunk;
use fubbik_db::repo::document::{self, Document, DocumentPatch, NewDocument};
use fubbik_db::repo::{space, tag};
use sqlx::PgPool;

use super::dto::{ImportResult, ImportStatus, RenderResult, SyncResult};
use super::render_markdown::{RenderOptions, render_markdown};
use super::split_markdown::{MarkdownSection, split_markdown};

fn hash_content(content: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Imports (or re-syncs, or no-ops on an unchanged hash) a single markdown
/// document — matching Node's `importDocument`
/// (`packages/api/src/documents/service.ts:45-159`), **default
/// (non-template) path only**.
///
/// Node's `importDocument` also accepts an optional `templateId` that
/// switches to a template-aware import branch
/// (`packages/api/src/documents/service.ts:63-116`, using `parseDocFile`
/// from `chunks/parse-docs.ts` and `extractFields` from
/// `templates/field-extraction.ts`). That branch is **not ported**:
///
/// - `POST /api/documents/import` never accepts or forwards a `templateId`
///   (`packages/api/src/documents/routes.ts:47-64` only reads
///   `sourcePath`/`content`/`spaceId` off the body) and no other caller in
///   the codebase (CLI, MCP, tests) invokes `importDocument` with one
///   either — the branch is dead code from every real entry point.
/// - It depends on `templates/field-extraction.ts`, already flagged
///   out of scope for this port's templates slice — see
///   `fubbik_db::repo::template`'s module doc comment ("the (unported, out
///   of scope for this slice) match/extraction engine").
///
/// Flagged for the human call rather than silently reimplemented.
pub async fn import_document(
    pool: &PgPool,
    user_id: &str,
    source_path: &str,
    raw_content: &str,
    space_id: Option<&str>,
) -> AppResult<ImportResult> {
    let content_hash = hash_content(raw_content);

    if let Some(existing) =
        document::find_by_source_path(pool, user_id, source_path, space_id).await?
    {
        if existing.content_hash == content_hash {
            let chunks = document::document_chunks(pool, user_id, &existing.id).await?;
            let first_chunk_id = chunks.first().map(|c| c.id.clone());
            return Ok(ImportResult {
                document: existing,
                created: 0,
                updated: 0,
                status: ImportStatus::Unchanged,
                first_chunk_id,
            });
        }

        let sync = sync_document(pool, user_id, &existing.id, raw_content, space_id).await?;
        let chunks = document::document_chunks(pool, user_id, &existing.id).await?;
        let first_chunk_id = chunks.first().map(|c| c.id.clone());
        return Ok(ImportResult {
            document: sync.document,
            created: sync.created,
            updated: sync.updated,
            status: sync.status,
            first_chunk_id,
        });
    }

    let split = split_markdown(raw_content, source_path);
    let doc_id = fubbik_db::new_id();

    let doc = document::create(
        pool,
        user_id,
        NewDocument {
            id: doc_id.clone(),
            title: split.title.clone(),
            source_path: source_path.to_string(),
            content_hash,
            description: split.description.clone(),
            space_id: space_id.map(str::to_string),
            split_level: Some(split.split_level),
        },
    )
    .await?;

    let tag_ids = if split.tags.is_empty() {
        Vec::new()
    } else {
        document::resolve_tag_ids(pool, user_id, &split.tags).await?
    };
    let space_ids: Vec<String> = space_id.map(|s| vec![s.to_string()]).unwrap_or_default();

    let mut first_chunk_id: Option<String> = None;
    let created = split.sections.len() as i32;
    for section in &split.sections {
        let chunk_id = fubbik_db::new_id();
        if first_chunk_id.is_none() {
            first_chunk_id = Some(chunk_id.clone());
        }
        document::insert_section_chunk(
            pool,
            user_id,
            &chunk_id,
            &section.title,
            &section.content,
            &doc_id,
            section.order,
        )
        .await?;

        if !tag_ids.is_empty() {
            tag::set_chunk_tags(pool, user_id, &chunk_id, &tag_ids).await?;
        }
        if !space_ids.is_empty() {
            space::set_chunk_spaces(pool, user_id, &chunk_id, &space_ids).await?;
        }
    }

    Ok(ImportResult {
        document: doc,
        created,
        updated: 0,
        status: ImportStatus::Created,
        first_chunk_id,
    })
}

/// Re-splits `raw_content` against an existing document, diffing sections
/// by normalised (trim + lowercase) title — matching Node's `syncDocument`
/// (`packages/api/src/documents/service.ts:161-239`) exactly, including
/// two divergent-looking-but-faithful details:
///
/// - **The returned `document` is the pre-sync snapshot.** Node fetches
///   `doc` once at the top and returns that same binding at the end,
///   despite calling `updateDocumentRepo` (which changes `title`/
///   `contentHash`/`description`/`splitLevel` in the database) in between
///   — `doc` is never reassigned. This port's response is byte-for-byte
///   the same shape: stale `document`, fresh DB row.
/// - **Orphaned sections' `document_order` is never actually cleared** —
///   see `fubbik_db::repo::document::touch_chunk`'s doc comment for the
///   Node bug this reproduces.
pub async fn sync_document(
    pool: &PgPool,
    user_id: &str,
    document_id: &str,
    raw_content: &str,
    space_id: Option<&str>,
) -> AppResult<SyncResult> {
    let doc = document::find_by_id(pool, user_id, document_id)
        .await?
        .ok_or_else(|| AppError::NotFound("document".to_string()))?;

    let content_hash = hash_content(raw_content);
    if doc.content_hash == content_hash {
        return Ok(SyncResult {
            document: doc,
            created: 0,
            updated: 0,
            status: ImportStatus::Unchanged,
        });
    }

    let split = split_markdown(raw_content, &doc.source_path);
    let existing_chunks = document::document_chunks(pool, user_id, document_id).await?;

    let normalize = |t: &str| t.trim().to_lowercase();
    let existing_by_title: std::collections::HashMap<String, &Chunk> = existing_chunks
        .iter()
        .map(|c| (normalize(&c.title), c))
        .collect();
    let mut matched_ids: std::collections::HashSet<String> = std::collections::HashSet::new();

    let mut created = 0i32;
    let mut updated = 0i32;

    let tag_ids = if split.tags.is_empty() {
        Vec::new()
    } else {
        document::resolve_tag_ids(pool, user_id, &split.tags).await?
    };
    let space_ids: Vec<String> = space_id.map(|s| vec![s.to_string()]).unwrap_or_default();

    for section in &split.sections {
        let key = normalize(&section.title);
        if let Some(existing) = existing_by_title.get(&key).copied() {
            matched_ids.insert(existing.id.clone());
            if existing.content != section.content || existing.document_order != Some(section.order)
            {
                document::update_section_chunk(
                    pool,
                    user_id,
                    &existing.id,
                    &section.content,
                    section.order,
                )
                .await?;
                updated += 1;
            }
        } else {
            let chunk_id = fubbik_db::new_id();
            document::insert_section_chunk(
                pool,
                user_id,
                &chunk_id,
                &section.title,
                &section.content,
                document_id,
                section.order,
            )
            .await?;

            if !tag_ids.is_empty() {
                tag::set_chunk_tags(pool, user_id, &chunk_id, &tag_ids).await?;
            }
            if !space_ids.is_empty() {
                space::set_chunk_spaces(pool, user_id, &chunk_id, &space_ids).await?;
            }
            created += 1;
        }
    }

    // Flag sections whose heading disappeared as stale (don't delete).
    for existing in &existing_chunks {
        if matched_ids.contains(&existing.id) {
            continue;
        }
        document::touch_chunk(pool, user_id, &existing.id).await?;
        let current_tags = tag::tags_for_chunk(pool, user_id, &existing.id).await?;
        let mut tag_names: Vec<String> = current_tags.into_iter().map(|t| t.name).collect();
        if !tag_names.iter().any(|n| n == "stale") {
            tag_names.push("stale".to_string());
            let stale_tag_ids = document::resolve_tag_ids(pool, user_id, &tag_names).await?;
            tag::set_chunk_tags(pool, user_id, &existing.id, &stale_tag_ids).await?;
        }
    }

    document::update(
        pool,
        user_id,
        document_id,
        DocumentPatch {
            title: Some(split.title.clone()),
            content_hash: Some(content_hash),
            description: split.description.clone(),
            split_level: Some(split.split_level),
        },
    )
    .await?;

    Ok(SyncResult {
        document: doc,
        created,
        updated,
        status: ImportStatus::Synced,
    })
}

/// Matches Node's `renderDocument` (`packages/api/src/documents/service.ts:241-278`).
pub async fn render_document(
    pool: &PgPool,
    user_id: &str,
    document_id: &str,
) -> AppResult<RenderResult> {
    let doc = document::find_by_id(pool, user_id, document_id)
        .await?
        .ok_or_else(|| AppError::NotFound("document".to_string()))?;

    let chunks = document::document_chunks(pool, user_id, document_id).await?;
    if chunks.is_empty() {
        let markdown = format!("# {}\n", doc.title);
        return Ok(RenderResult {
            document: doc,
            markdown,
        });
    }

    let tags = tag::tags_for_chunk(pool, user_id, &chunks[0].id).await?;
    let tag_names: Vec<String> = tags.into_iter().map(|t| t.name).collect();

    let first_chunk = &chunks[0];
    let scope = match &first_chunk.scope.0 {
        serde_json::Value::Object(map) if !map.is_empty() => Some(&first_chunk.scope.0),
        _ => None,
    };

    let sections: Vec<MarkdownSection> = chunks
        .iter()
        .enumerate()
        .map(|(i, c)| MarkdownSection {
            title: c.title.clone(),
            content: c.content.clone(),
            order: c.document_order.unwrap_or(i as i32),
            rationale: c.rationale.clone(),
            alternatives: c.alternatives.as_ref().map(|a| a.0.clone()),
            consequences: c.consequences.clone(),
        })
        .collect();

    let markdown = render_markdown(RenderOptions {
        title: &doc.title,
        doc_type: Some(first_chunk.chunk_type.as_str()),
        tags: &tag_names,
        scope,
        split_level: doc.split_level.unwrap_or(2),
        sections: &sections,
        source_path: Some(&doc.source_path),
    });

    Ok(RenderResult {
        document: doc,
        markdown,
    })
}

/// Matches Node's `getDocument` (`packages/api/src/documents/service.ts:288-295`)
/// — see `super::dto::DocumentDetail` for the flattened-shape note.
pub async fn get_document(
    pool: &PgPool,
    user_id: &str,
    document_id: &str,
) -> AppResult<super::dto::DocumentDetail> {
    let doc = document::find_by_id(pool, user_id, document_id)
        .await?
        .ok_or_else(|| AppError::NotFound("document".to_string()))?;
    let chunks = document::document_chunks(pool, user_id, document_id).await?;
    Ok(super::dto::DocumentDetail::new(doc, chunks))
}

pub async fn list_documents_with_tags(
    pool: &PgPool,
    user_id: &str,
    space_id: Option<&str>,
) -> AppResult<Vec<fubbik_db::repo::document::DocumentWithTagsItem>> {
    document::list_with_tags(pool, user_id, space_id).await
}

/// Matches Node's `searchDocuments` (`packages/api/src/documents/service.ts:297-299`),
/// which always calls the repo with a hardcoded limit of `20` — there is no
/// route-level override.
pub async fn search_documents(
    pool: &PgPool,
    user_id: &str,
    q: &str,
    space_id: Option<&str>,
) -> AppResult<Vec<fubbik_db::repo::document::DocumentSearchResult>> {
    document::search_chunks(pool, user_id, q, 20, space_id).await
}

/// Matches Node's `removeDocument`
/// (`packages/api/src/documents/service.ts:301-307`). A single guard:
/// `document::delete`'s own `WHERE id = $1 AND user_id = $2` — there is no
/// redundant pre-check here duplicating it (see `document::find_by_id`'s
/// doc comment on why this port scopes cross-user access in SQL directly,
/// one guard per operation).
pub async fn remove_document(
    pool: &PgPool,
    user_id: &str,
    document_id: &str,
) -> AppResult<Document> {
    document::delete(pool, user_id, document_id)
        .await?
        .ok_or_else(|| AppError::NotFound("document".to_string()))
}
