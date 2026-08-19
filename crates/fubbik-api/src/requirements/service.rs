//! Port of `packages/api/src/requirements/service.ts` and
//! `packages/api/src/requirements/batch-service.ts`.

use fubbik_core::error::{AppError, AppResult};
use fubbik_db::repo::requirement::{
    self, BulkPatch, NewRequirement, Requirement, RequirementChunkLink, RequirementPatch,
    RequirementStats, RequirementStep,
};
use fubbik_db::repo::{chunk, space, use_case};
use sqlx::PgPool;

use super::cross_ref::cross_reference_steps;
use super::dto::{
    self, BatchCreateBody, BatchCreateResponse, BatchCreatedRequirement, BatchUseCaseCreated,
    BulkActionBody, BulkActionKind, CreateRequirementBody, ListRequirementsQuery,
    ListRequirementsResponse, RequirementDetail, RequirementWithWarnings, StepVocabularyWarning,
    UpdateRequirementBody,
};
use super::error::{RequirementError, RequirementResult};
use super::validator::{StepError, validate_steps};
use crate::vocabulary::parser::{VocabEntry, parse_step_text};

async fn get_vocabulary_warnings(
    pool: &PgPool,
    user_id: &str,
    steps: &[RequirementStep],
    space_id: Option<&str>,
) -> Vec<StepVocabularyWarning> {
    let Some(space_id) = space_id else {
        return vec![];
    };
    let vocab = match fubbik_db::repo::vocabulary::list(pool, user_id, space_id).await {
        Ok(v) => v,
        Err(_) => return vec![],
    };
    let entries: Vec<VocabEntry> = vocab
        .into_iter()
        .map(|v| VocabEntry {
            word: v.word,
            category: v.category,
            expects: v.expects.map(|j| j.0),
        })
        .collect();

    let mut warnings = Vec::new();
    for (i, step) in steps.iter().enumerate() {
        let result = parse_step_text(&step.text, &entries);
        for w in result.warnings {
            warnings.push(StepVocabularyWarning {
                position: w.position,
                warning_type: w.warning_type,
                word: w.word,
                message: w.message,
                step: i as i32,
            });
        }
    }
    warnings
}

fn step_validation_error(errors: Vec<StepError>) -> RequirementError {
    RequirementError::StepValidation(
        serde_json::to_value(errors).expect("StepError always serializes"),
    )
}

pub async fn list_requirements(
    pool: &PgPool,
    user_id: &str,
    query: ListRequirementsQuery,
) -> AppResult<ListRequirementsResponse> {
    let limit = query
        .limit
        .as_deref()
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(50)
        .min(100);
    let offset = query
        .offset
        .as_deref()
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(0);

    let params = requirement::ListParams {
        space_id: query.space_id.as_deref(),
        use_case_id: query.use_case_id.as_deref(),
        status: query.status.as_deref(),
        priority: query.priority.as_deref(),
        origin: query.origin.map(|o| o.as_str()),
        review_status: query.review_status.map(|r| r.as_str()),
        search: query.search.as_deref(),
        limit,
        offset,
    };

    let requirements = requirement::list(pool, user_id, &params).await?;
    let total = requirement::count(pool, user_id, &params).await?;
    Ok(ListRequirementsResponse {
        requirements,
        total,
    })
}

pub async fn get_requirement(
    pool: &PgPool,
    user_id: &str,
    id: &str,
) -> AppResult<RequirementDetail> {
    let req = requirement::find_by_id(pool, user_id, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Requirement".into()))?;
    let chunks = requirement::get_chunks(pool, user_id, id).await?;
    Ok(RequirementDetail::new(req, chunks))
}

/// Mirrors Node's `createRequirement` (`packages/api/src/requirements/
/// service.ts:99-135`). Unlike Node — whose repo-level `createRequirement`
/// is a bare insert with no ownership check on `spaceId`/`useCaseId` at
/// all — this pre-checks both here, the same "check through the parent
/// before calling create" shape `collections::service::create` uses,
/// matching `fubbik_db::repo::requirement::create`'s own accepted
/// ownership-guard divergence (see that function's doc comment).
pub async fn create_requirement(
    pool: &PgPool,
    user_id: &str,
    body: CreateRequirementBody,
) -> RequirementResult<RequirementWithWarnings> {
    let errors = validate_steps(&body.steps);
    if !errors.is_empty() {
        return Err(step_validation_error(errors));
    }

    if let Some(space_id) = &body.space_id
        && space::find_by_id(pool, user_id, space_id).await?.is_none()
    {
        return Err(AppError::NotFound("Space".into()).into());
    }
    if let Some(use_case_id) = &body.use_case_id
        && use_case::find_by_id(pool, user_id, use_case_id)
            .await?
            .is_none()
    {
        return Err(AppError::NotFound("Use case".into()).into());
    }

    let origin = body.origin.map(|o| o.as_str()).unwrap_or("human");
    let review_status = if origin == "ai" { "draft" } else { "approved" };

    let requirement = requirement::create(
        pool,
        user_id,
        NewRequirement {
            title: body.title,
            description: body.description,
            steps: body.steps.clone(),
            priority: body.priority.map(|p| p.as_str().to_string()),
            space_id: body.space_id,
            use_case_id: body.use_case_id,
            origin: origin.to_string(),
            review_status: review_status.to_string(),
        },
    )
    .await?
    .ok_or_else(|| AppError::NotFound("Requirement".into()))?;

    let warnings = cross_reference_steps(pool, user_id, &body.steps).await;
    let vocabulary_warnings =
        get_vocabulary_warnings(pool, user_id, &body.steps, requirement.space_id.as_deref()).await;

    Ok(RequirementWithWarnings {
        requirement,
        warnings,
        vocabulary_warnings,
    })
}

/// Mirrors Node's `updateRequirement` (`packages/api/src/requirements/
/// service.ts:137-187`). Node's body has no `status` field at all (see
/// `RequirementPatch`'s doc comment) — the "flag failing chunks on status
/// change" branch that exists in Node's repo-layer function signature is
/// therefore dead code no route can reach, and is not reproduced here.
pub async fn update_requirement(
    pool: &PgPool,
    user_id: &str,
    id: &str,
    body: UpdateRequirementBody,
) -> RequirementResult<RequirementWithWarnings> {
    let existing = requirement::find_by_id(pool, user_id, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Requirement".into()))?;

    if let Some(steps) = &body.steps {
        let errors = validate_steps(steps);
        if !errors.is_empty() {
            return Err(step_validation_error(errors));
        }
    }

    let reviewed_by = body.review_status.map(|_| user_id.to_string());
    let reviewed_at = body
        .review_status
        .map(|_| fubbik_db::timestamp::UtcTimestamp(chrono::Utc::now().naive_utc()));

    let patch = RequirementPatch {
        title: body.title,
        description: body.description,
        steps: body.steps.clone(),
        priority: body.priority.map(|p| p.map(|v| v.as_str().to_string())),
        space_id: body.space_id.clone(),
        use_case_id: body.use_case_id,
        origin: body.origin.map(|o| o.as_str().to_string()),
        review_status: body.review_status.map(|r| r.as_str().to_string()),
        reviewed_by,
        reviewed_at,
    };

    let requirement = requirement::update(pool, user_id, id, patch)
        .await?
        .ok_or_else(|| AppError::NotFound("Requirement".into()))?;

    let warnings = match &body.steps {
        Some(steps) => cross_reference_steps(pool, user_id, steps).await,
        None => vec![],
    };

    let space_id_for_vocab: Option<String> = match body.space_id {
        Some(v) => v,
        None => existing.space_id,
    };
    let vocabulary_warnings = match &body.steps {
        Some(steps) => {
            get_vocabulary_warnings(pool, user_id, steps, space_id_for_vocab.as_deref()).await
        }
        None => vec![],
    };

    Ok(RequirementWithWarnings {
        requirement,
        warnings,
        vocabulary_warnings,
    })
}

pub async fn delete_requirement(pool: &PgPool, user_id: &str, id: &str) -> AppResult<()> {
    if requirement::find_by_id(pool, user_id, id).await?.is_none() {
        return Err(AppError::NotFound("Requirement".into()));
    }
    requirement::delete(pool, user_id, id).await?;
    Ok(())
}

/// Mirrors Node's `updateStatus` (`packages/api/src/requirements/
/// service.ts:199-217`): setting status to `"failing"` flags every linked
/// chunk via `staleness::flag_requirement_failing`, fire-and-forget (its
/// return value is discarded, same as Node discards `flagRequirementFailing`'s).
pub async fn update_status(
    pool: &PgPool,
    user_id: &str,
    id: &str,
    status: dto::Status,
) -> AppResult<Requirement> {
    let updated = requirement::update_status(pool, user_id, id, status.as_str())
        .await?
        .ok_or_else(|| AppError::NotFound("Requirement".into()))?;

    if status == dto::Status::Failing {
        let chunks = requirement::get_chunks(pool, user_id, id).await?;
        let chunk_ids: Vec<String> = chunks.into_iter().map(|c| c.id).collect();
        fubbik_db::repo::staleness::flag_requirement_failing(
            pool,
            user_id,
            id,
            &updated.title,
            &chunk_ids,
        )
        .await?;
    }

    Ok(updated)
}

/// Mirrors Node's `setChunks` (`packages/api/src/requirements/service.ts:
/// 219-238`): every `chunk_id` is verified to exist and belong to `user_id`
/// *before* the join is replaced — a single missing/foreign chunk id fails
/// the whole call with a 404 naming it, matching Node's `Effect.all`
/// over per-chunk `getChunkById` checks exactly (nothing is deleted or
/// inserted on a rejected call).
pub async fn set_chunks(
    pool: &PgPool,
    user_id: &str,
    requirement_id: &str,
    chunk_ids: Vec<String>,
) -> AppResult<Vec<RequirementChunkLink>> {
    if requirement::find_by_id(pool, user_id, requirement_id)
        .await?
        .is_none()
    {
        return Err(AppError::NotFound("Requirement".into()));
    }
    for chunk_id in &chunk_ids {
        if chunk::find_by_id(pool, user_id, chunk_id).await?.is_none() {
            return Err(AppError::NotFound(format!("Chunk {chunk_id}")));
        }
    }
    requirement::set_chunks(pool, user_id, requirement_id, &chunk_ids).await
}

pub async fn get_stats(
    pool: &PgPool,
    user_id: &str,
    space_id: Option<&str>,
) -> AppResult<RequirementStats> {
    requirement::stats(pool, user_id, space_id).await
}

/// Mirrors Node's `bulkAction` (`packages/api/src/requirements/service.ts:
/// 244-261`): returns the bare row count `bulkUpdateRequirements`/
/// `bulkDeleteRequirements` return — **not** wrapped in an object — same
/// as Node's route, which resolves the handler to that raw number and lets
/// Elysia serialise it as a bare JSON number.
pub async fn bulk_action(pool: &PgPool, user_id: &str, body: BulkActionBody) -> AppResult<i64> {
    let count = match body.action {
        BulkActionKind::SetStatus => {
            requirement::bulk_update(
                pool,
                user_id,
                &body.ids,
                BulkPatch {
                    status: body.status.map(|s| s.as_str().to_string()),
                    use_case_id: None,
                },
            )
            .await?
        }
        BulkActionKind::SetUseCase => {
            requirement::bulk_update(
                pool,
                user_id,
                &body.ids,
                BulkPatch {
                    status: None,
                    use_case_id: body.use_case_id,
                },
            )
            .await?
        }
        BulkActionKind::Delete => requirement::bulk_delete(pool, user_id, &body.ids).await?,
    };
    Ok(count as i64)
}

/// Mirrors Node's `reorderRequirements` (`packages/api/src/requirements/
/// service.ts:263-278`). Returns `requirement_ids.len()` unconditionally,
/// matching Node's `setRequirementOrder`, which returns
/// `requirementIds.length` regardless of how many rows the loop's
/// `UPDATE`s actually touched.
pub async fn reorder_requirements(
    pool: &PgPool,
    user_id: &str,
    requirement_ids: Vec<String>,
) -> AppResult<usize> {
    let existing = requirement::find_by_ids(pool, user_id, &requirement_ids).await?;
    if existing.len() != requirement_ids.len() {
        return Err(AppError::Validation(
            "Some requirements not found or not owned by user".into(),
        ));
    }

    let use_case_ids: std::collections::HashSet<Option<String>> =
        existing.into_iter().map(|r| r.use_case_id).collect();
    if use_case_ids.len() > 1 {
        return Err(AppError::Validation(
            "Requirements must belong to the same use case".into(),
        ));
    }

    requirement::set_order(pool, user_id, &requirement_ids).await?;
    Ok(requirement_ids.len())
}

pub async fn export_requirement(
    pool: &PgPool,
    user_id: &str,
    id: &str,
    format: dto::Format,
) -> AppResult<String> {
    let req = requirement::find_by_id(pool, user_id, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Requirement".into()))?;
    Ok(super::export::export_one(
        &req.title,
        &req.steps.0,
        format.as_str(),
    ))
}

pub async fn export_all(
    pool: &PgPool,
    user_id: &str,
    query: dto::ExportAllQuery,
) -> AppResult<String> {
    let params = requirement::ListParams {
        space_id: query.space_id.as_deref(),
        use_case_id: None,
        status: None,
        priority: None,
        origin: None,
        review_status: None,
        search: None,
        limit: 10_000,
        offset: 0,
    };
    let requirements = requirement::list(pool, user_id, &params).await?;
    let separator = if query.format == dto::Format::Markdown {
        "\n\n---\n\n"
    } else {
        "\n\n"
    };
    Ok(requirements
        .iter()
        .map(|r| super::export::export_one(&r.title, &r.steps.0, query.format.as_str()))
        .collect::<Vec<_>>()
        .join(separator))
}

/// Mirrors Node's `batchCreateRequirements`
/// (`packages/api/src/requirements/batch-service.ts:23-102`): validates
/// every requirement's steps upfront (a single validation failure on any
/// entry rejects the whole batch with no partial writes), then resolves
/// `useCaseName`/`parentUseCaseName` to ids — creating new use cases as
/// needed, cached by name within one call so the same name is never
/// created twice in one batch — before creating each requirement with
/// `origin: "ai"`, `reviewStatus: "draft"` unconditionally.
pub async fn batch_create_requirements(
    pool: &PgPool,
    user_id: &str,
    body: BatchCreateBody,
) -> RequirementResult<BatchCreateResponse> {
    let mut all_step_errors: Vec<dto::BatchStepError> = Vec::new();
    for (i, req) in body.requirements.iter().enumerate() {
        for err in validate_steps(&req.steps) {
            all_step_errors.push(dto::BatchStepError {
                index: i as i32,
                step: err.step,
                error: err.error,
            });
        }
    }
    if !all_step_errors.is_empty() {
        return Err(RequirementError::StepValidation(
            serde_json::to_value(all_step_errors).expect("BatchStepError always serializes"),
        ));
    }

    let mut name_to_id: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    let mut use_cases_created: Vec<BatchUseCaseCreated> = Vec::new();

    // Boxed so the function can call itself for the optional parent name —
    // matches Node's `resolveUseCaseName`, a closure that recurses one
    // level deep for `parentUseCaseName`.
    async fn resolve_use_case_name(
        pool: &PgPool,
        user_id: &str,
        space_id: Option<&str>,
        name: &str,
        parent_id: Option<&str>,
        name_to_id: &mut std::collections::HashMap<String, String>,
        use_cases_created: &mut Vec<BatchUseCaseCreated>,
    ) -> AppResult<String> {
        if let Some(id) = name_to_id.get(name) {
            return Ok(id.clone());
        }
        if let Some(existing) = use_case::find_by_name(pool, user_id, name).await? {
            name_to_id.insert(name.to_string(), existing.id.clone());
            return Ok(existing.id);
        }
        let created = use_case::create(
            pool,
            user_id,
            use_case::NewUseCase {
                name: name.to_string(),
                description: None,
                space_id: space_id.map(String::from),
                parent_id: parent_id.map(String::from),
            },
        )
        .await?
        .ok_or_else(|| AppError::NotFound("Space".into()))?;
        name_to_id.insert(name.to_string(), created.id.clone());
        use_cases_created.push(BatchUseCaseCreated {
            id: created.id.clone(),
            name: created.name,
            parent_id: created.parent_id,
        });
        Ok(created.id)
    }

    let mut created: Vec<BatchCreatedRequirement> = Vec::new();

    for req in &body.requirements {
        let mut use_case_id = req.use_case_id.clone();

        if use_case_id.is_none()
            && let Some(use_case_name) = &req.use_case_name
        {
            let parent_id = if let Some(parent_name) = &req.parent_use_case_name {
                Some(
                    resolve_use_case_name(
                        pool,
                        user_id,
                        body.space_id.as_deref(),
                        parent_name,
                        None,
                        &mut name_to_id,
                        &mut use_cases_created,
                    )
                    .await?,
                )
            } else {
                None
            };
            use_case_id = Some(
                resolve_use_case_name(
                    pool,
                    user_id,
                    body.space_id.as_deref(),
                    use_case_name,
                    parent_id.as_deref(),
                    &mut name_to_id,
                    &mut use_cases_created,
                )
                .await?,
            );
        }

        let result = requirement::create(
            pool,
            user_id,
            NewRequirement {
                title: req.title.clone(),
                description: req.description.clone(),
                steps: req.steps.clone(),
                priority: req.priority.map(|p| p.as_str().to_string()),
                space_id: body.space_id.clone(),
                use_case_id,
                origin: "ai".to_string(),
                review_status: "draft".to_string(),
            },
        )
        .await?
        .ok_or_else(|| AppError::NotFound("Requirement".into()))?;

        created.push(BatchCreatedRequirement {
            id: result.id,
            title: result.title,
            use_case_id: result.use_case_id,
        });
    }

    Ok(BatchCreateResponse {
        created: created.len(),
        requirements: created,
        use_cases_created,
    })
}
