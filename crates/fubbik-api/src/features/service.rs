//! Business logic for the `features` domain, porting
//! `packages/api/src/features/service.ts`.
//!
//! Ownership is enforced twice on purpose. Every repository query in
//! `fubbik_db::repo::feature` filters on `user_id` in the SQL itself, and
//! the pre-checks below stay in place on top of that — not as belt and
//! braces for their own sake, but because they are what produce Node's
//! *distinct* failures: a missing feature 404s with `"Feature"`, a missing
//! chunk with `"Chunk"`, a missing delta with `"Delta"`. Drop the
//! pre-checks and the SQL guard still protects the data, but the responses
//! collapse into one shape.

use fubbik_core::error::{AppError, AppResult};
use fubbik_db::repo::chunk;
use fubbik_db::repo::feature::{
    self, ChunkFeatureDelta, DELTA_ALLOWED_FIELDS, DeltaWithChunk, DeltaWithFeature, Feature,
    FeatureListItem, FeaturePatch, ListParams,
};
use sqlx::PgPool;

use super::dto::{CreateFeatureBody, FeatureDetail, ListFeaturesQuery, UpdateFeatureBody};

/// Ports `validateDelta` (`packages/api/src/features/service.ts:33-46`)
/// exactly, including the order of its two checks: unknown fields are
/// reported *before* emptiness, so `{}` and `{bogus: 1}` produce different
/// messages. Both are `ValidationError` → 400.
///
/// A delta is sparse by construction — only the keys the caller sends are
/// stored, and only those are applied on merge. Nothing here fills in
/// absent fields.
fn validate_delta(delta: &serde_json::Map<String, serde_json::Value>) -> AppResult<()> {
    let invalid: Vec<&str> = delta
        .keys()
        .map(String::as_str)
        .filter(|k| !DELTA_ALLOWED_FIELDS.contains(k))
        .collect();
    if !invalid.is_empty() {
        return Err(AppError::Validation(format!(
            "Invalid delta fields: {}. Allowed: {}",
            invalid.join(", "),
            DELTA_ALLOWED_FIELDS.join(", ")
        )));
    }
    if delta.is_empty() {
        return Err(AppError::Validation(
            "Delta must contain at least one field".to_string(),
        ));
    }
    Ok(())
}

pub async fn list_features(
    pool: &PgPool,
    user_id: &str,
    query: ListFeaturesQuery,
) -> AppResult<Vec<FeatureListItem>> {
    feature::list(
        pool,
        user_id,
        &ListParams {
            space_id: query.space_id.as_deref(),
            status: query.status.as_deref(),
            search: query.search.as_deref(),
        },
    )
    .await
}

/// Auto-assigns `max(priority) + 1` when the client omits `priority`,
/// matching Node — and, like Node, does **not** deduplicate an explicitly
/// supplied one. `(user_id, priority)` is uniquely indexed, so a colliding
/// explicit priority raises a unique violation and surfaces as a 500 in
/// both implementations. Node also skips any name-uniqueness check on
/// create (it only guards renames), so a duplicate name is likewise a 500.
pub async fn create_feature(
    pool: &PgPool,
    user_id: &str,
    body: CreateFeatureBody,
) -> AppResult<Feature> {
    let priority = match body.priority {
        Some(p) => p,
        None => feature::max_priority(pool, user_id).await? + 1,
    };

    let id = fubbik_db::new_id();
    let created = feature::create(
        pool,
        &id,
        user_id,
        &body.name,
        body.description.as_deref(),
        priority,
        body.color.as_deref(),
    )
    .await?;

    // Node: `if (body.spaceIds && body.spaceIds.length > 0)` — an empty
    // array is a no-op on create (contrast the PATCH path, where an empty
    // array clears).
    if let Some(space_ids) = body.space_ids.filter(|s| !s.is_empty()) {
        feature::set_spaces(pool, &id, user_id, &space_ids).await?;
    }
    Ok(created)
}

/// `{ feature, spaces, deltas }` — 404s on a feature that is missing or
/// another user's before either sub-query runs.
pub async fn get_feature_detail(
    pool: &PgPool,
    feature_id: &str,
    user_id: &str,
) -> AppResult<FeatureDetail> {
    let found = feature::find_by_id(pool, feature_id, user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Feature".into()))?;

    Ok(FeatureDetail {
        feature: found,
        spaces: feature::spaces_for_feature(pool, feature_id, user_id).await?,
        deltas: feature::deltas_for_feature(pool, feature_id, user_id).await?,
    })
}

/// `GET /features/{id}/deltas` is Node's `getFeatureDetail(...).deltas`
/// (`packages/api/src/features/routes.ts:150-158`) — it therefore 404s on
/// an unowned feature and, incidentally, still pays for the `spaces` query
/// it throws away. Reproduced by going through the same call.
pub async fn feature_deltas(
    pool: &PgPool,
    feature_id: &str,
    user_id: &str,
) -> AppResult<Vec<DeltaWithChunk>> {
    Ok(get_feature_detail(pool, feature_id, user_id).await?.deltas)
}

/// The rename guard runs *before* the update, and is a `ValidationError`
/// (400) rather than a 409 — Node's message is
/// `Feature "<name>" already exists`. Without it the `(user_id, name)`
/// unique index would still reject the write, but as a 500.
pub async fn update_feature(
    pool: &PgPool,
    feature_id: &str,
    user_id: &str,
    body: UpdateFeatureBody,
) -> AppResult<Feature> {
    if let Some(name) = body.name.as_deref()
        && feature::name_conflict(pool, feature_id, user_id, name).await?
    {
        return Err(AppError::Validation(format!(
            "Feature \"{name}\" already exists"
        )));
    }

    let updated = feature::update(
        pool,
        feature_id,
        user_id,
        FeaturePatch {
            name: body.name,
            description: body.description,
            priority: body.priority,
            status: body.status.map(|s| s.as_str().to_string()),
            color: body.color,
        },
    )
    .await?
    .ok_or_else(|| AppError::NotFound("Feature".into()))?;

    // Node: `if (spaceIds !== undefined)` — an empty array here *does*
    // clear the associations, unlike on create.
    if let Some(space_ids) = body.space_ids {
        feature::set_spaces(pool, feature_id, user_id, &space_ids).await?;
    }
    Ok(updated)
}

pub async fn delete_feature(pool: &PgPool, feature_id: &str, user_id: &str) -> AppResult<()> {
    feature::delete(pool, feature_id, user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Feature".into()))?;
    Ok(())
}

/// Ports `reorderFeature` (`packages/api/src/features/service.ts:132-143`).
///
/// The renumbering is exactly Node's, and it is worth being precise about
/// what it does: it shifts **every** feature of the user at or above
/// `new_priority` up by one — including the feature being moved, when that
/// feature already sits at or above the target — and then writes
/// `new_priority` onto the target. Nothing is renumbered downward, so
/// repeated reorders inflate priorities without ever compacting them, and
/// the vacated slot is left empty. That is Node's behaviour, kept.
///
/// A no-op reorder (`existing.priority == new_priority`) returns the
/// existing row untouched without shifting anything — the early return is
/// load-bearing, since shifting first would move the feature off its own
/// target.
///
/// **The shift itself can fail**, see
/// `feature::shift_priorities_up`'s doc comment: a contiguous run of
/// priorities makes the bulk `UPDATE` violate the `(user_id, priority)`
/// unique index mid-statement, in Node and here alike.
pub async fn reorder_feature(
    pool: &PgPool,
    feature_id: &str,
    user_id: &str,
    new_priority: i32,
) -> AppResult<Feature> {
    let existing = feature::find_by_id(pool, feature_id, user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Feature".into()))?;

    if existing.priority == new_priority {
        return Ok(existing);
    }

    feature::shift_priorities_up(pool, user_id, new_priority).await?;
    feature::update(
        pool,
        feature_id,
        user_id,
        FeaturePatch {
            priority: Some(new_priority),
            ..Default::default()
        },
    )
    .await?
    .ok_or_else(|| AppError::NotFound("Feature".into()))
}

pub async fn get_active_features(pool: &PgPool, user_id: &str) -> AppResult<Vec<String>> {
    feature::active_feature_ids(pool, user_id).await
}

/// An empty list clears the active set with **no validation at all** —
/// Node short-circuits before ever listing the user's features
/// (`packages/api/src/features/service.ts:150-152`), which its own unit
/// test pins (`service.test.ts:164-171`).
///
/// A non-empty list is checked against the caller's own features and any
/// stranger produces `Features not found: <ids>` (400), *before* the
/// existing set is touched — a partially-valid list changes nothing.
pub async fn set_active_features(
    pool: &PgPool,
    user_id: &str,
    feature_ids: Vec<String>,
) -> AppResult<()> {
    if feature_ids.is_empty() {
        return feature::set_active_features(pool, user_id, &feature_ids).await;
    }

    let owned = feature::list(pool, user_id, &ListParams::default()).await?;
    let invalid: Vec<&str> = feature_ids
        .iter()
        .map(String::as_str)
        .filter(|id| !owned.iter().any(|f| f.id == *id))
        .collect();
    if !invalid.is_empty() {
        return Err(AppError::Validation(format!(
            "Features not found: {}",
            invalid.join(", ")
        )));
    }

    feature::set_active_features(pool, user_id, &feature_ids).await
}

/// Ports `mergeFeature` (`packages/api/src/features/service.ts:192-229`).
///
/// Re-merging is rejected with a 400 (`Feature is already merged`) rather
/// than being idempotent. A feature with no deltas takes the cheap path —
/// a bare status flip, no transaction, no version snapshots — which is
/// exactly what Node's `deltas.length === 0` branch does and what its own
/// test asserts (`service.test.ts:209-222`: `mergeFeatureDeltas` must
/// *not* be called).
///
/// Otherwise everything happens inside `feature::merge_feature_deltas`'s
/// single transaction. The delta list is read here, before the
/// transaction, exactly as Node reads it before calling
/// `mergeFeatureDeltas` — so a delta created in the gap is deleted by the
/// merge without being applied. That race is Node's; it is not introduced
/// here and not papered over.
pub async fn merge_feature(pool: &PgPool, feature_id: &str, user_id: &str) -> AppResult<()> {
    let found = feature::find_by_id(pool, feature_id, user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Feature".into()))?;

    if found.status == "merged" {
        return Err(AppError::Validation("Feature is already merged".into()));
    }

    let deltas = feature::deltas_for_feature(pool, feature_id, user_id).await?;
    if deltas.is_empty() {
        feature::update(
            pool,
            feature_id,
            user_id,
            FeaturePatch {
                status: Some("merged".into()),
                ..Default::default()
            },
        )
        .await?;
        return Ok(());
    }

    let pairs: Vec<(String, serde_json::Value)> = deltas
        .into_iter()
        .map(|d| (d.chunk_id, d.delta.0))
        .collect();
    // The returned chunk ids are what Node hands to its fire-and-forget
    // `enrichChunk` loop; there is no enrichment service in this port yet.
    feature::merge_feature_deltas(pool, feature_id, user_id, &pairs).await?;
    Ok(())
}

pub async fn deltas_for_chunk(
    pool: &PgPool,
    chunk_id: &str,
    user_id: &str,
) -> AppResult<Vec<DeltaWithFeature>> {
    feature::deltas_for_chunk(pool, chunk_id, user_id).await
}

/// Validation order is Node's and is observable: an invalid delta is
/// rejected (400) *before* the feature is looked up, so a bad delta aimed
/// at a non-existent feature is a 400, not a 404. Feature is then checked
/// before chunk, so a request that is wrong about both reports `"Feature"`.
/// `service.test.ts:326-342` pins both orderings.
pub async fn upsert_delta(
    pool: &PgPool,
    chunk_id: &str,
    feature_id: &str,
    user_id: &str,
    delta: serde_json::Map<String, serde_json::Value>,
) -> AppResult<ChunkFeatureDelta> {
    validate_delta(&delta)?;

    if feature::find_by_id(pool, feature_id, user_id)
        .await?
        .is_none()
    {
        return Err(AppError::NotFound("Feature".into()));
    }
    // NB: `chunk::find_by_id` takes `(pool, user_id, id)`, not `(pool, id,
    // user_id)` like `feature::find_by_id` — both are `&str`, so the
    // compiler cannot catch a swap.
    if chunk::find_by_id(pool, user_id, chunk_id).await?.is_none() {
        return Err(AppError::NotFound("Chunk".into()));
    }

    let id = fubbik_db::new_id();
    feature::upsert_delta(
        pool,
        &id,
        chunk_id,
        feature_id,
        user_id,
        &serde_json::Value::Object(delta),
    )
    .await?
    // Unreachable behind the two pre-checks above; the SQL guard is what
    // makes it representable at all, and a silent `Ok` would be worse than
    // a 404 if the two ever disagreed.
    .ok_or_else(|| AppError::NotFound("Delta".into()))
}

/// The feature check comes first and 404s with `"Feature"`; only a feature
/// that exists but has no delta on this chunk yields `"Delta"`.
pub async fn delete_delta(
    pool: &PgPool,
    chunk_id: &str,
    feature_id: &str,
    user_id: &str,
) -> AppResult<()> {
    if feature::find_by_id(pool, feature_id, user_id)
        .await?
        .is_none()
    {
        return Err(AppError::NotFound("Feature".into()));
    }
    feature::delete_delta(pool, chunk_id, feature_id, user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Delta".into()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate_delta;

    fn map(json: serde_json::Value) -> serde_json::Map<String, serde_json::Value> {
        json.as_object().unwrap().clone()
    }

    #[test]
    fn rejects_empty_delta() {
        let err = validate_delta(&map(serde_json::json!({}))).unwrap_err();
        assert!(err.to_string().contains("at least one field"));
    }

    #[test]
    fn rejects_fields_outside_the_allowed_seven() {
        // `tags` and `embedding` are real chunk columns, and both are
        // rejected — the allow-list is not "any chunk field".
        for bad in ["tags", "embedding", "userId"] {
            let err = validate_delta(&map(serde_json::json!({ bad: "x" }))).unwrap_err();
            assert!(
                err.to_string().contains(bad),
                "message must name the offending field `{bad}`"
            );
        }
    }

    #[test]
    fn accepts_all_seven_allowed_fields_and_any_subset() {
        let all = serde_json::json!({
            "title": "T", "content": "C", "type": "document",
            "rationale": "R", "alternatives": ["A"], "consequences": "Q",
            "summary": "S"
        });
        validate_delta(&map(all)).unwrap();
        validate_delta(&map(serde_json::json!({ "title": "only" }))).unwrap();
    }

    #[test]
    fn unknown_field_beats_emptiness_in_the_message() {
        // Order matters: Node checks unknown keys first, so a delta that is
        // both non-empty and wrong reports the field, never "at least one".
        let err = validate_delta(&map(serde_json::json!({ "nope": 1 }))).unwrap_err();
        assert!(
            err.to_string()
                .starts_with("validation failed: Invalid delta fields: nope")
        );
    }
}
