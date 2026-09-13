//! Atomic reconciliation of generated documentation. No source program runs here.
use std::collections::HashSet;

use fubbik_core::error::{AppError, AppResult};
use fubbik_core::source_docs::SourceManifest;
use sqlx::{PgPool, Row};

use super::{chunk, chunk_version, document, space};

#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    pub document_id: String,
    pub created: usize,
    pub updated: usize,
    pub unchanged: usize,
    pub missing: usize,
    pub conflicts: Vec<String>,
    pub diagnostics: Vec<String>,
}

pub async fn import(
    pool: &PgPool,
    user_id: &str,
    space_id: &str,
    manifest: &SourceManifest,
) -> AppResult<ImportReport> {
    manifest.validate().map_err(AppError::Validation)?;
    let mut tx = pool.begin().await?;
    // Serialize imports in a space, including two first-time imports where
    // there is no document row to lock yet. The same query verifies ownership.
    let owned: Option<String> =
        sqlx::query_scalar("SELECT id FROM space WHERE id = $1 AND user_id = $2 FOR NO KEY UPDATE")
            .bind(space_id)
            .bind(user_id)
            .fetch_optional(&mut *tx)
            .await?;
    if owned.is_none() {
        return Err(AppError::NotFound("space".into()));
    }
    let language = manifest.language.as_str();
    let existing: Option<String> = sqlx::query_scalar(
        "SELECT document_id FROM source_documentation WHERE user_id=$1 AND space_id=$2 AND project=$3 AND language=$4",
    ).bind(user_id).bind(space_id).bind(&manifest.project).bind(language)
        .fetch_optional(&mut *tx).await?;
    let document_id = if let Some(id) = existing {
        id
    } else {
        let id = crate::new_id();
        document::create_in(
            &mut tx,
            user_id,
            document::NewDocument {
                id: id.clone(),
                title: format!("{} {} API", manifest.project, language),
                source_path: manifest.source_path(),
                content_hash: String::new(),
                description: Some("Generated from source documentation comments".into()),
                space_id: Some(space_id.into()),
                split_level: Some(2),
            },
        )
        .await?;
        sqlx::query("INSERT INTO source_documentation (document_id,user_id,space_id,project,language,extractor) VALUES ($1,$2,$3,$4,$5,$6)")
            .bind(&id).bind(user_id).bind(space_id).bind(&manifest.project).bind(language).bind(&manifest.extractor)
            .execute(&mut *tx).await?;
        id
    };
    let mut report = ImportReport {
        document_id: document_id.clone(),
        created: 0,
        updated: 0,
        unchanged: 0,
        missing: 0,
        conflicts: vec![],
        diagnostics: manifest.diagnostics.clone(),
    };
    let mut symbols: Vec<_> = manifest.symbols.iter().collect();
    symbols.sort_by(|a, b| a.key.cmp(&b.key));
    // Free generated positions before assigning the new order. Otherwise a
    // newly inserted symbol can collide with an existing section's unique slot.
    sqlx::query("UPDATE chunk SET document_order=NULL WHERE document_id=$1 AND id IN (SELECT chunk_id FROM source_documentation_symbol WHERE document_id=$1)")
        .bind(&document_id).execute(&mut *tx).await?;
    let first_order: i32 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(document_order),-1)+1 FROM chunk WHERE document_id=$1",
    )
    .bind(&document_id)
    .fetch_one(&mut *tx)
    .await?;
    for (order, symbol) in symbols.iter().enumerate() {
        let order = first_order
            .checked_add(order as i32)
            .ok_or_else(|| AppError::Validation("document section order overflow".into()))?;
        let content = symbol.markdown(manifest.language);
        let previous = sqlx::query("SELECT chunk_id,generated_title,generated_content,missing FROM source_documentation_symbol WHERE document_id=$1 AND symbol_key=$2")
            .bind(&document_id).bind(&symbol.key).fetch_optional(&mut *tx).await?;
        let chunk_id = if let Some(previous) = previous {
            let id: String = previous.get("chunk_id");
            chunk::lock_for_update_in(&mut tx, user_id, &id).await?;
            let current = chunk::find_by_id_in(&mut tx, user_id, &id)
                .await?
                .ok_or_else(|| AppError::NotFound("source chunk".into()))?;
            let old_title: String = previous.get("generated_title");
            let old_content: String = previous.get("generated_content");
            sqlx::query("UPDATE chunk SET document_order=$2 WHERE id=$1")
                .bind(&id)
                .bind(order)
                .execute(&mut *tx)
                .await?;
            if current.title != old_title || current.content != old_content {
                report.conflicts.push(symbol.key.clone());
                continue;
            }
            let missing: bool = previous.get("missing");
            if current.title != symbol.title || current.content != content || missing {
                chunk_version::snapshot_in(&mut tx, &current, Some("source-docs")).await?;
                chunk::update_in(
                    &mut tx,
                    user_id,
                    &id,
                    chunk::ChunkPatch {
                        title: Some(symbol.title.clone()),
                        content: Some(content.clone()),
                        document_order: Some(order),
                        ..Default::default()
                    },
                )
                .await?;
                sqlx::query(
                    "UPDATE chunk SET embedding=NULL, embedding_updated_at=NULL WHERE id=$1",
                )
                .bind(&id)
                .execute(&mut *tx)
                .await?;
                if missing {
                    sqlx::query("UPDATE chunk SET archived_at=NULL WHERE id=$1")
                        .bind(&id)
                        .execute(&mut *tx)
                        .await?;
                }
                report.updated += 1;
            } else {
                report.unchanged += 1;
                continue;
            }
            id
        } else {
            let created = chunk::create_in(
                &mut tx,
                user_id,
                chunk::NewChunk {
                    title: symbol.title.clone(),
                    content: content.clone(),
                    chunk_type: "reference".into(),
                    rationale: None,
                    alternatives: None,
                    consequences: None,
                    origin: "human".into(),
                    review_status: "approved".into(),
                    document_id: Some(document_id.clone()),
                    document_order: Some(order),
                },
            )
            .await?;
            space::set_chunk_spaces_in(&mut tx, user_id, &created.id, &[space_id.into()]).await?;
            report.created += 1;
            created.id
        };
        sqlx::query("INSERT INTO source_documentation_symbol (document_id,symbol_key,chunk_id,generated_title,generated_content) VALUES ($1,$2,$3,$4,$5) ON CONFLICT (document_id,symbol_key) DO UPDATE SET generated_title=$4,generated_content=$5,missing=false")
            .bind(&document_id).bind(&symbol.key).bind(&chunk_id).bind(&symbol.title).bind(&content).execute(&mut *tx).await?;
        // Own only our source reference, preserving user-added references.
        sqlx::query(
            "DELETE FROM chunk_file_ref WHERE chunk_id=$1 AND anchor=$2 AND relation='documents'",
        )
        .bind(&chunk_id)
        .bind(&symbol.key)
        .execute(&mut *tx)
        .await?;
        sqlx::query("INSERT INTO chunk_file_ref (id,chunk_id,path,anchor,relation) VALUES ($1,$2,$3,$4,'documents')")
            .bind(crate::new_id()).bind(&chunk_id).bind(&symbol.path).bind(&symbol.key).execute(&mut *tx).await?;
    }
    if manifest.complete {
        let seen: HashSet<_> = manifest.symbols.iter().map(|s| s.key.as_str()).collect();
        let rows = sqlx::query("SELECT symbol_key,chunk_id,generated_title,generated_content FROM source_documentation_symbol WHERE document_id=$1 AND NOT missing ORDER BY symbol_key")
            .bind(&document_id).fetch_all(&mut *tx).await?;
        for row in rows {
            let key: String = row.get("symbol_key");
            if seen.contains(key.as_str()) {
                continue;
            }
            let id: String = row.get("chunk_id");
            chunk::lock_for_update_in(&mut tx, user_id, &id).await?;
            let current = chunk::find_by_id_in(&mut tx, user_id, &id)
                .await?
                .ok_or_else(|| AppError::NotFound("source chunk".into()))?;
            if current.title != row.get::<String, _>("generated_title")
                || current.content != row.get::<String, _>("generated_content")
            {
                report.conflicts.push(key);
                continue;
            }
            sqlx::query("UPDATE source_documentation_symbol SET missing=true WHERE chunk_id=$1")
                .bind(&id)
                .execute(&mut *tx)
                .await?;
            sqlx::query("UPDATE chunk SET archived_at=now(),updated_at=now() WHERE id=$1")
                .bind(&id)
                .execute(&mut *tx)
                .await?;
            report.missing += 1;
        }
    }
    sqlx::query("UPDATE source_documentation SET extractor=$2 WHERE document_id=$1")
        .bind(&document_id)
        .bind(&manifest.extractor)
        .execute(&mut *tx)
        .await?;
    // Hash accepted generated content, so conflicts do not record an
    // unaccepted manifest as current. Ordering keeps the hash reproducible.
    if report.created + report.updated + report.missing > 0 {
        sqlx::query("UPDATE document SET content_hash=md5(COALESCE((SELECT string_agg(symbol_key || generated_title || generated_content, '' ORDER BY symbol_key) FROM source_documentation_symbol WHERE document_id=$1 AND NOT missing),'')), updated_at=now() WHERE id=$1")
            .bind(&document_id).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(report)
}
