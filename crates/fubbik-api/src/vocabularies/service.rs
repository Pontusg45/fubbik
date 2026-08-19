//! Port of `packages/api/src/vocabularies/service.ts`.
//!
//! ## Two flagged Node behaviours, reproduced rather than fixed
//!
//! 1. **`spaceId` widens the list instead of filtering it.** Both list
//!    queries are `or(builtIn = true, userId = ..., spaceId = ...)`
//!    (`packages/db/src/repository/vocabulary-catalog.ts:18-25,31-38`), so
//!    `GET /api/chunk-types?spaceId=X` returns the caller's own rows *plus*
//!    every row scoped to space `X` — including rows another user created
//!    against that space. That reads like a bug (`and` was probably
//!    intended, or at minimum a space-membership check), but it is Node's
//!    actual contract and is reproduced verbatim. Flagged, not silently
//!    "corrected".
//!
//! 2. **`codebaseId` on the create bodies is dead code.** Node's
//!    `CreateChunkTypeBody`/`CreateRelationBody` declare
//!    `codebaseId?: string | null`
//!    (`service.ts:35,100`), but (a) the route body schemas
//!    (`routes.ts:7-15,26-35`) have no such field, so Elysia strips it
//!    before the service ever sees it, and (b) the repository reads
//!    `row.spaceId`, not `row.codebaseId`
//!    (`vocabulary-catalog.ts:70,140`), so even a hand-constructed call
//!    would drop it. Net effect: rows created through this API always have
//!    `space_id IS NULL`. This port has no `spaceId`/`codebaseId` field at
//!    all, which matches the observable behaviour exactly.

use fubbik_core::error::{AppError, AppResult};
use fubbik_db::repo::chunk_type::{self, ChunkType, ChunkTypePatch, NewChunkType};
use fubbik_db::repo::connection_relation::{
    self, ConnectionRelation, ConnectionRelationPatch, NewConnectionRelation,
};
use sqlx::PgPool;

use super::dto::{
    CreateChunkTypeBody, CreateConnectionRelationBody, UpdateChunkTypeBody,
    UpdateConnectionRelationBody,
};

/// Node's `SLUG_RE = /^[a-z0-9][a-z0-9_-]{0,40}$/`
/// (`packages/api/src/vocabularies/service.ts:38`), hand-rolled because
/// this workspace carries no regex dependency.
///
/// Read carefully: the first character must be `[a-z0-9]`, and the
/// *remaining* characters (0..=40 of them) may each be `[a-z0-9_-]`. So the
/// maximum total length is 41, matching the `maxLength: 41` on the Elysia
/// schema, and the empty string fails (no first character).
fn is_valid_slug(id: &str) -> bool {
    let mut chars = id.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_ascii_lowercase() && !first.is_ascii_digit() {
        return false;
    }
    let rest: Vec<char> = chars.collect();
    if rest.len() > 40 {
        return false;
    }
    rest.iter()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '_' || *c == '-')
}

/// The message is copied verbatim from Node
/// (`packages/api/src/vocabularies/service.ts:44`) and is shared by both
/// catalogs, which use the identical `ValidationError` text.
fn validate_slug(id: &str) -> AppResult<()> {
    if is_valid_slug(id) {
        Ok(())
    } else {
        Err(AppError::Validation(
            "id must be a lowercase slug (letters, digits, - or _, max 41 chars)".into(),
        ))
    }
}

/// An empty `spaceId` query string is `undefined` to Node
/// (`ctx.query.spaceId || undefined`, `routes.ts:53,96`), not the empty
/// string — so `?spaceId=` must behave exactly like omitting the parameter.
fn normalise_space_id(space_id: Option<&str>) -> Option<&str> {
    space_id.filter(|s| !s.is_empty())
}

// --- chunk_type -------------------------------------------------------------

pub async fn list_chunk_types(
    pool: &PgPool,
    user_id: &str,
    space_id: Option<&str>,
) -> AppResult<Vec<ChunkType>> {
    chunk_type::list(pool, user_id, normalise_space_id(space_id)).await
}

/// Mirrors Node's `createChunkType`
/// (`packages/api/src/vocabularies/service.ts:40-53`): slug check, then an
/// *unscoped* duplicate-id check (the slug is the table's primary key, so
/// it is global — another user's custom type with the same slug blocks
/// yours), then the insert.
pub async fn create_chunk_type(
    pool: &PgPool,
    user_id: &str,
    body: CreateChunkTypeBody,
) -> AppResult<ChunkType> {
    validate_slug(&body.id)?;

    if chunk_type::find_by_id(pool, &body.id).await?.is_some() {
        return Err(AppError::Validation(format!(
            "chunk type \"{}\" already exists",
            body.id
        )));
    }

    chunk_type::create(
        pool,
        user_id,
        NewChunkType {
            id: body.id,
            label: body.label,
            description: body.description,
            icon: body.icon,
            color: body.color,
            examples: body.examples,
            display_order: body.display_order,
        },
    )
    .await
}

/// Mirrors Node's `updateChunkType`
/// (`packages/api/src/vocabularies/service.ts:55-70`) exactly: unscoped
/// existence lookup (404), built-in rejection (400, message copied
/// verbatim), then the user-scoped `UPDATE` whose `None` result — the id
/// exists but is not the caller's — also maps to 404.
///
/// The built-in check is observable, not redundant: without it a PATCH of a
/// built-in row would still fail (`chunk_type::update`'s `WHERE user_id =
/// $2` can never match a `NULL` `user_id`), but with 404 instead of 400.
/// `tests/vocabularies.rs::update_chunk_type_rejects_built_in_with_400`
/// pins the status code.
pub async fn update_chunk_type(
    pool: &PgPool,
    user_id: &str,
    id: &str,
    body: UpdateChunkTypeBody,
) -> AppResult<ChunkType> {
    let found = chunk_type::find_by_id(pool, id)
        .await?
        .ok_or_else(|| AppError::NotFound("ChunkType".into()))?;

    if found.built_in {
        return Err(AppError::Validation(
            "builtin chunk types cannot be edited".into(),
        ));
    }

    chunk_type::update(
        pool,
        user_id,
        id,
        ChunkTypePatch {
            label: body.label,
            description: body.description,
            icon: body.icon,
            color: body.color,
            examples: body.examples,
            display_order: body.display_order,
        },
    )
    .await?
    .ok_or_else(|| AppError::NotFound("ChunkType".into()))
}

/// Mirrors Node's `deleteChunkType`
/// (`packages/api/src/vocabularies/service.ts:72-87`) — same shape as
/// `update_chunk_type`, different message.
pub async fn delete_chunk_type(pool: &PgPool, user_id: &str, id: &str) -> AppResult<()> {
    let found = chunk_type::find_by_id(pool, id)
        .await?
        .ok_or_else(|| AppError::NotFound("ChunkType".into()))?;

    if found.built_in {
        return Err(AppError::Validation(
            "builtin chunk types cannot be deleted".into(),
        ));
    }

    if chunk_type::delete(pool, user_id, id).await? {
        Ok(())
    } else {
        Err(AppError::NotFound("ChunkType".into()))
    }
}

// --- connection_relation ----------------------------------------------------

pub async fn list_connection_relations(
    pool: &PgPool,
    user_id: &str,
    space_id: Option<&str>,
) -> AppResult<Vec<ConnectionRelation>> {
    connection_relation::list(pool, user_id, normalise_space_id(space_id)).await
}

/// Mirrors Node's `createConnectionRelation`
/// (`packages/api/src/vocabularies/service.ts:103-116`). Note the duplicate
/// message says `relation "..."`, not `chunk type "..."` — the two
/// catalogs' messages differ and are copied separately.
pub async fn create_connection_relation(
    pool: &PgPool,
    user_id: &str,
    body: CreateConnectionRelationBody,
) -> AppResult<ConnectionRelation> {
    validate_slug(&body.id)?;

    if connection_relation::find_by_id(pool, &body.id)
        .await?
        .is_some()
    {
        return Err(AppError::Validation(format!(
            "relation \"{}\" already exists",
            body.id
        )));
    }

    connection_relation::create(
        pool,
        user_id,
        NewConnectionRelation {
            id: body.id,
            label: body.label,
            description: body.description,
            arrow_style: body.arrow_style.map(|v| v.as_str().to_string()),
            direction: body.direction.map(|v| v.as_str().to_string()),
            color: body.color,
            inverse_of_id: body.inverse_of_id,
            display_order: body.display_order,
        },
    )
    .await
}

/// Mirrors Node's `updateConnectionRelation`
/// (`packages/api/src/vocabularies/service.ts:118-133`).
pub async fn update_connection_relation(
    pool: &PgPool,
    user_id: &str,
    id: &str,
    body: UpdateConnectionRelationBody,
) -> AppResult<ConnectionRelation> {
    let found = connection_relation::find_by_id(pool, id)
        .await?
        .ok_or_else(|| AppError::NotFound("ConnectionRelation".into()))?;

    if found.built_in {
        return Err(AppError::Validation(
            "builtin relations cannot be edited".into(),
        ));
    }

    connection_relation::update(
        pool,
        user_id,
        id,
        ConnectionRelationPatch {
            label: body.label,
            description: body.description,
            arrow_style: body.arrow_style.map(|v| v.as_str().to_string()),
            direction: body.direction.map(|v| v.as_str().to_string()),
            color: body.color,
            inverse_of_id: body.inverse_of_id,
            display_order: body.display_order,
        },
    )
    .await?
    .ok_or_else(|| AppError::NotFound("ConnectionRelation".into()))
}

/// Mirrors Node's `deleteConnectionRelation`
/// (`packages/api/src/vocabularies/service.ts:135-150`).
pub async fn delete_connection_relation(pool: &PgPool, user_id: &str, id: &str) -> AppResult<()> {
    let found = connection_relation::find_by_id(pool, id)
        .await?
        .ok_or_else(|| AppError::NotFound("ConnectionRelation".into()))?;

    if found.built_in {
        return Err(AppError::Validation(
            "builtin relations cannot be deleted".into(),
        ));
    }

    if connection_relation::delete(pool, user_id, id).await? {
        Ok(())
    } else {
        Err(AppError::NotFound("ConnectionRelation".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::is_valid_slug;

    #[test]
    fn slug_rules_match_nodes_regex() {
        // Accepted.
        assert!(is_valid_slug("a"));
        assert!(is_valid_slug("0"));
        assert!(is_valid_slug("runbook"));
        assert!(is_valid_slug("run_book-2"));
        assert!(is_valid_slug(&"a".repeat(41)), "41 chars is the cap");

        // Rejected.
        assert!(!is_valid_slug(""), "empty has no leading char");
        assert!(!is_valid_slug("_leading"), "must start [a-z0-9]");
        assert!(!is_valid_slug("-leading"));
        assert!(!is_valid_slug("Upper"));
        assert!(!is_valid_slug("has space"));
        assert!(!is_valid_slug("has.dot"));
        assert!(!is_valid_slug(&"a".repeat(42)), "42 chars is one too many");
    }
}
