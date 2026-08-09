use std::collections::HashSet;

use fubbik_core::error::{AppError, AppResult};
use fubbik_db::repo::activity::Activity;
use fubbik_db::repo::plan::{
    self, CompletedAtPatch, ListFilter, Plan, PlanAnalyzeItem, PlanExternalLink, PlanListRow,
    PlanRequirement,
};
use sqlx::PgPool;

use super::dto::{
    AddRequirementBody, AnalyzeGrouped, CreateAnalyzeItemBody, CreateLinkBody, CreatePlanBody,
    PlanDetail, ReorderAnalyzeItemsBody, ReorderRequirementsBody, TaskDetail,
    UpdateAnalyzeItemBody, UpdatePlanBody, acceptance_criteria_for_write,
    normalize_acceptance_criteria,
};

/// Node's `VALID_ANALYZE_KINDS` (`packages/api/src/plans/service.ts:8`).
/// `plan_analyze_item.kind` is unconstrained free text at the DB and schema
/// level — see `fubbik_db::repo::plan::analyze`'s module doc — so this is
/// the *only* place a `kind` value is ever rejected. Deliberately not a
/// Rust enum on any DTO: Node's Elysia body schema is `t.String()` and
/// accepts any string at deserialisation, rejecting only here (mirroring
/// `analyze.ts`'s `isAnalyzeKind` guard).
const VALID_ANALYZE_KINDS: [&str; 5] = ["chunk", "file", "risk", "assumption", "question"];

fn validate_analyze_kind(kind: &str) -> AppResult<()> {
    if VALID_ANALYZE_KINDS.contains(&kind) {
        Ok(())
    } else {
        Err(AppError::Validation(format!(
            "Invalid analyze kind: {kind}"
        )))
    }
}

/// Node's `VALID_STATUSES` (`packages/api/src/plans/service.ts:7`).
/// `plan.status` is unconstrained free text at the DB and schema level — see
/// `fubbik_db::repo::plan`'s module doc comment — so this is the *only*
/// place a `status` value is ever rejected. Deliberately not a Rust enum on
/// any DTO: Node's Elysia body schema is `t.Optional(t.String())` and
/// accepts any string at deserialisation, rejecting only here.
const VALID_STATUSES: [&str; 6] = [
    "draft",
    "analyzing",
    "ready",
    "in_progress",
    "completed",
    "archived",
];

fn validate_status(status: &str) -> AppResult<()> {
    if VALID_STATUSES.contains(&status) {
        Ok(())
    } else {
        Err(AppError::Validation(format!("Invalid status: {status}")))
    }
}

/// Mirrors Node's `listPlans` (`packages/api/src/plans/service.ts:36-49`):
/// validates an explicit `status` filter the same way `update` validates a
/// written one, then delegates to the rollup-carrying list query.
pub async fn list(pool: &PgPool, user_id: &str, filter: ListFilter) -> AppResult<Vec<PlanListRow>> {
    if let Some(status) = &filter.status {
        validate_status(status)?;
    }
    plan::list_with_rollups(pool, user_id, filter).await
}

/// Mirrors Node's `getPlan` (`packages/api/src/plans/service.ts:58-62`):
/// 404 if the plan doesn't exist or isn't the caller's (divergence #13 —
/// Node's own `getPlan` has no ownership check; `plan::find_by_id` adds
/// one in SQL).
pub async fn get_plan(pool: &PgPool, user_id: &str, id: &str) -> AppResult<Plan> {
    plan::find_by_id(pool, user_id, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Plan".into()))
}

/// Mirrors Node's `getPlanDetail` (`packages/api/src/plans/service.ts:68-98`):
/// the plan itself, its linked requirements, analyze items grouped into the
/// five fixed kinds, tasks (acceptance criteria normalised, chunks
/// attached), and task dependencies.
pub async fn get_detail(pool: &PgPool, user_id: &str, id: &str) -> AppResult<PlanDetail> {
    let found = get_plan(pool, user_id, id).await?;
    let requirements = plan::list_requirements(pool, user_id, id).await?;
    let analyze_items = plan::list_analyze_items(pool, user_id, id).await?;
    let tasks = plan::list_tasks(pool, user_id, id).await?;
    let dependencies = plan::list_task_dependencies(pool, user_id, id).await?;

    let mut task_details = Vec::with_capacity(tasks.len());
    for t in tasks {
        let chunks = plan::list_task_chunks_with_titles(pool, &t.id).await?;
        task_details.push(TaskDetail {
            id: t.id,
            plan_id: t.plan_id,
            title: t.title,
            description: t.description,
            acceptance_criteria: normalize_acceptance_criteria(&t.acceptance_criteria.0),
            status: t.status,
            order: t.order,
            created_at: t.created_at,
            updated_at: t.updated_at,
            metadata: t.metadata.0,
            chunks,
        });
    }

    Ok(PlanDetail {
        plan: found,
        requirements,
        analyze: AnalyzeGrouped::from_items(analyze_items),
        tasks: task_details,
        dependencies,
    })
}

/// Mirrors Node's `createPlan` (`packages/api/src/plans/service.ts:124-157`):
/// rejects a whitespace-only title (trimmed before both the check and the
/// insert, same as `workspaces::service::create`), then optionally links
/// requirements and creates initial tasks. `metadata`, `requirementIds`,
/// and `tasks` aren't supported by `plan::create` (Task 3's given
/// interface) directly, so they're applied as follow-up calls against the
/// freshly created row — each one already scoped to `user_id`, so this
/// can't act on anyone else's plan even if `created.id` were somehow
/// guessable.
pub async fn create(pool: &PgPool, user_id: &str, body: CreatePlanBody) -> AppResult<Plan> {
    let title = body.title.trim();
    if title.is_empty() {
        return Err(AppError::Validation("Title is required".into()));
    }

    let mut created = plan::create(
        pool,
        user_id,
        title,
        body.description.as_deref(),
        body.space_id.as_deref(),
    )
    .await?;

    if let Some(metadata) = body.metadata {
        created = plan::apply_patch(
            pool,
            user_id,
            &created.id,
            None,
            None,
            None,
            None,
            Some(metadata),
            CompletedAtPatch::Unchanged,
        )
        .await?
        .ok_or_else(|| AppError::NotFound("Plan".into()))?;
    }

    if let Some(requirement_ids) = &body.requirement_ids {
        for rid in requirement_ids {
            plan::add_requirement(pool, user_id, &created.id, rid).await?;
        }
    }

    if let Some(tasks) = &body.tasks {
        for t in tasks {
            let criteria =
                acceptance_criteria_for_write(t.acceptance_criteria.as_deref().unwrap_or(&[]));
            plan::create_task(
                pool,
                user_id,
                &created.id,
                &t.title,
                t.description.as_deref(),
                criteria,
            )
            .await?;
        }
    }

    Ok(created)
}

/// Mirrors Node's `updatePlan` (`packages/api/src/plans/service.ts:167-188`):
/// validates a provided `status`, 404s up front if the plan isn't the
/// caller's, computes the `completed_at` side effect from the *existing*
/// row's status vs. the incoming one, and delegates the actual write to
/// `plan::apply_patch` — see that function's doc comment for why
/// `plan::update` (Task 3) isn't sufficient here (no way to express
/// tri-state clearing).
pub async fn update(
    pool: &PgPool,
    user_id: &str,
    id: &str,
    body: UpdatePlanBody,
) -> AppResult<Plan> {
    if let Some(status) = &body.status {
        validate_status(status)?;
    }
    let existing = get_plan(pool, user_id, id).await?;

    let completed_at = match &body.status {
        Some(s) if s == "completed" && existing.status != "completed" => CompletedAtPatch::SetNow,
        Some(s) if s != "completed" && existing.status == "completed" => CompletedAtPatch::Clear,
        _ => CompletedAtPatch::Unchanged,
    };

    let description = body.description.as_ref().map(|d| d.as_deref());
    let space_id = body.space_id.as_ref().map(|s| s.as_deref());

    plan::apply_patch(
        pool,
        user_id,
        id,
        body.title.as_deref(),
        description,
        body.status.as_deref(),
        space_id,
        body.metadata,
        completed_at,
    )
    .await?
    .ok_or_else(|| AppError::NotFound("Plan".into()))
}

/// Mirrors Node's `deletePlan` (`packages/api/src/plans/service.ts:190-195`):
/// 404 pre-check, then delete — `plan::delete`'s own `AND user_id = $2`
/// guard is defense-in-depth, same belt-and-suspenders shape used
/// throughout this port.
pub async fn delete(pool: &PgPool, user_id: &str, id: &str) -> AppResult<()> {
    get_plan(pool, user_id, id).await?;
    if plan::delete(pool, user_id, id).await? {
        Ok(())
    } else {
        Err(AppError::NotFound("Plan".into()))
    }
}

/// Mirrors Node's `duplicatePlan` (`packages/api/src/plans/service.ts:51-56`):
/// 404 if the source plan doesn't exist / isn't the caller's before
/// attempting the deep copy — `plan::duplicate`'s own ownership guard on
/// the source `SELECT` is defense-in-depth, proven load-bearing in Task 3's
/// `tests/plan.rs::duplicate_is_user_scoped`.
pub async fn duplicate(pool: &PgPool, user_id: &str, source_id: &str) -> AppResult<Plan> {
    get_plan(pool, user_id, source_id).await?;
    plan::duplicate(pool, user_id, source_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Plan".into()))
}

/// Mirrors Node's `GET /plans/:id/activity` handler
/// (`packages/api/src/plans/routes.ts:154-182`): 404-checks the plan, then
/// merges plan-level activity events (`entityType: "plan", entityId:
/// planId`, limit 100) with task-level events (`entityType: "plan_task"`,
/// limit 200, filtered down to this plan's own task ids), sorts the union
/// by `createdAt` descending, and takes the first 100.
///
/// **Known gap, not attempted here**: no domain in this port (plans
/// included) currently writes `activity_log` rows on mutation — Node's
/// `createActivity` calls scattered through `plans/routes.ts` have no Rust
/// equivalent yet in *any* ported domain (`fubbik_db::repo::activity` is a
/// read-only surface today; confirmed by grep — see this crate's
/// `activity` module). Wiring activity writes into every mutating route is
/// out of scope for "the 10 core plans endpoints" and is consistent with
/// the rest of the port's current state, not a plans-specific omission.
/// This endpoint's read/merge/sort/truncate logic is nonetheless complete
/// and correct against whatever rows exist.
pub async fn get_activity(pool: &PgPool, user_id: &str, id: &str) -> AppResult<Vec<Activity>> {
    get_plan(pool, user_id, id).await?;

    let tasks = plan::list_tasks(pool, user_id, id).await?;
    let task_ids: HashSet<String> = tasks.into_iter().map(|t| t.id).collect();

    let plan_events = plan::list_activity_by_entity(pool, user_id, "plan", Some(id), 100).await?;
    let task_events = plan::list_activity_by_entity(pool, user_id, "plan_task", None, 200).await?;

    let mut merged: Vec<Activity> = plan_events
        .into_iter()
        .chain(
            task_events
                .into_iter()
                .filter(|e| task_ids.contains(&e.entity_id)),
        )
        .collect();
    merged.sort_by_key(|e| std::cmp::Reverse(e.created_at));
    merged.truncate(100);
    Ok(merged)
}

/// Mirrors Node's `GET /plans/:id/links` (`packages/api/src/plans/routes.ts:184-191`).
pub async fn list_links(
    pool: &PgPool,
    user_id: &str,
    id: &str,
) -> AppResult<Vec<PlanExternalLink>> {
    get_plan(pool, user_id, id).await?;
    plan::list_links(pool, user_id, id).await
}

/// Mirrors Node's `POST /plans/:id/links` (`packages/api/src/plans/routes.ts:192-215`):
/// `system` defaults to `"url"`, `label` to `null`, applied here (after the
/// field is known absent) rather than via a serde default, matching
/// Elysia's `?? "url"` / `?? null`.
pub async fn add_link(
    pool: &PgPool,
    user_id: &str,
    id: &str,
    body: CreateLinkBody,
) -> AppResult<PlanExternalLink> {
    get_plan(pool, user_id, id).await?;
    let system = body.system.as_deref().unwrap_or("url");
    plan::add_link(pool, user_id, id, system, &body.url, body.label.as_deref())
        .await?
        .ok_or_else(|| AppError::NotFound("Plan".into()))
}

/// Mirrors Node's `DELETE /plans/:id/links/:linkId` (`packages/api/src/plans/routes.ts:216-224`).
pub async fn remove_link(pool: &PgPool, user_id: &str, id: &str, link_id: &str) -> AppResult<()> {
    get_plan(pool, user_id, id).await?;
    if plan::remove_link(pool, user_id, id, link_id).await? {
        Ok(())
    } else {
        Err(AppError::NotFound("PlanExternalLink".into()))
    }
}

// ── Requirement links ────────────────────────────────────────────────

/// Mirrors Node's `POST /plans/:id/requirements` (`requirements.ts:9-20`).
pub async fn add_requirement(
    pool: &PgPool,
    user_id: &str,
    id: &str,
    body: AddRequirementBody,
) -> AppResult<PlanRequirement> {
    get_plan(pool, user_id, id).await?;
    plan::add_requirement(pool, user_id, id, &body.requirement_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Plan".into()))
}

/// Mirrors Node's `DELETE /plans/:id/requirements/:requirementId`
/// (`requirements.ts:21-29`). Node's own repo call is a bare `void` with no
/// not-found signal (always `{ok:true}`); this port's plans domain already
/// diverges from that for link deletes (`remove_link`), so this follows the
/// same convention rather than being the one outlier — see
/// `plan::remove_requirement`'s doc comment.
pub async fn remove_requirement(
    pool: &PgPool,
    user_id: &str,
    id: &str,
    requirement_id: &str,
) -> AppResult<()> {
    get_plan(pool, user_id, id).await?;
    if plan::remove_requirement(pool, user_id, id, requirement_id).await? {
        Ok(())
    } else {
        Err(AppError::NotFound("PlanRequirement".into()))
    }
}

/// Mirrors Node's `POST /plans/:id/requirements/reorder` (`requirements.ts:30-42`).
pub async fn reorder_requirements(
    pool: &PgPool,
    user_id: &str,
    id: &str,
    body: ReorderRequirementsBody,
) -> AppResult<()> {
    get_plan(pool, user_id, id).await?;
    plan::reorder_requirements(pool, user_id, id, &body.requirement_ids).await
}

// ── Analyze items ────────────────────────────────────────────────────

/// Mirrors Node's `GET /plans/:id/analyze` (`analyze.ts:39-47`): the same
/// group-by-kind shape as the `analyze` field of `get_detail`'s envelope.
pub async fn list_analyze(pool: &PgPool, user_id: &str, id: &str) -> AppResult<AnalyzeGrouped> {
    get_plan(pool, user_id, id).await?;
    let items = plan::list_analyze_items(pool, user_id, id).await?;
    Ok(AnalyzeGrouped::from_items(items))
}

/// Mirrors Node's `POST /plans/:id/analyze` (`analyze.ts:49-78`): validates
/// `kind` against the five known values, then delegates to the repo
/// function, which itself raises `NotFound` when the plan isn't the
/// caller's (see `plan::create_analyze_item`'s doc comment).
pub async fn create_analyze_item(
    pool: &PgPool,
    user_id: &str,
    id: &str,
    body: CreateAnalyzeItemBody,
) -> AppResult<PlanAnalyzeItem> {
    validate_analyze_kind(&body.kind)?;
    get_plan(pool, user_id, id).await?;
    plan::create_analyze_item(
        pool,
        user_id,
        id,
        &body.kind,
        body.chunk_id.as_deref(),
        body.file_path.as_deref(),
        body.text.as_deref(),
        body.metadata,
    )
    .await
}

/// Mirrors Node's `PATCH /plans/:id/analyze/:itemId` (`analyze.ts:79-97`).
pub async fn update_analyze_item(
    pool: &PgPool,
    user_id: &str,
    id: &str,
    item_id: &str,
    body: UpdateAnalyzeItemBody,
) -> AppResult<PlanAnalyzeItem> {
    get_plan(pool, user_id, id).await?;
    plan::update_analyze_item(
        pool,
        user_id,
        id,
        item_id,
        body.text.as_deref(),
        body.metadata,
        body.chunk_id.as_deref(),
        body.file_path.as_deref(),
    )
    .await?
    .ok_or_else(|| AppError::NotFound("PlanAnalyzeItem".into()))
}

/// Mirrors Node's `DELETE /plans/:id/analyze/:itemId` (`analyze.ts:98-106`).
/// Same domain-wide "surface a real 404" convention as
/// `remove_link`/`remove_requirement` — see `plan::delete_analyze_item`'s
/// doc comment.
pub async fn delete_analyze_item(
    pool: &PgPool,
    user_id: &str,
    id: &str,
    item_id: &str,
) -> AppResult<()> {
    get_plan(pool, user_id, id).await?;
    if plan::delete_analyze_item(pool, user_id, id, item_id).await? {
        Ok(())
    } else {
        Err(AppError::NotFound("PlanAnalyzeItem".into()))
    }
}

/// Mirrors Node's `POST /plans/:id/analyze/reorder` (`analyze.ts:107-120`).
pub async fn reorder_analyze_items(
    pool: &PgPool,
    user_id: &str,
    id: &str,
    body: ReorderAnalyzeItemsBody,
) -> AppResult<()> {
    validate_analyze_kind(&body.kind)?;
    get_plan(pool, user_id, id).await?;
    plan::reorder_analyze_items(pool, user_id, id, &body.kind, &body.item_ids).await
}
