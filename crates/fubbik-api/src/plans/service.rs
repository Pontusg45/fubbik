use std::collections::HashSet;

use fubbik_core::error::{AppError, AppResult};
use fubbik_db::repo::activity::Activity;
use fubbik_db::repo::plan::{self, ListFilter, Plan};
use sqlx::PgPool;

use super::db::{self as plan_detail, CompletedAtPatch, PlanExternalLink, PlanListRow};
use super::dto::{
    AnalyzeGrouped, CreateLinkBody, CreatePlanBody, PlanDetail, TaskDetail, UpdatePlanBody,
    acceptance_criteria_for_write, normalize_acceptance_criteria,
};

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
    plan_detail::list_with_rollups(pool, user_id, filter).await
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
    let requirements = plan_detail::list_requirements(pool, user_id, id).await?;
    let analyze_items = plan_detail::list_analyze_items(pool, user_id, id).await?;
    let tasks = plan_detail::list_tasks(pool, user_id, id).await?;
    let dependencies = plan_detail::list_task_dependencies(pool, user_id, id).await?;

    let mut task_details = Vec::with_capacity(tasks.len());
    for t in tasks {
        let chunks = plan_detail::list_task_chunks_with_titles(pool, &t.id).await?;
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
        created = plan_detail::apply_patch(
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
            plan_detail::add_requirement(pool, user_id, &created.id, rid).await?;
        }
    }

    if let Some(tasks) = &body.tasks {
        for t in tasks {
            let criteria =
                acceptance_criteria_for_write(t.acceptance_criteria.as_deref().unwrap_or(&[]));
            plan_detail::create_task(
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
/// `plan_detail::apply_patch` — see that function's doc comment for why
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

    plan_detail::apply_patch(
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

    let tasks = plan_detail::list_tasks(pool, user_id, id).await?;
    let task_ids: HashSet<String> = tasks.into_iter().map(|t| t.id).collect();

    let plan_events =
        plan_detail::list_activity_by_entity(pool, user_id, "plan", Some(id), 100).await?;
    let task_events =
        plan_detail::list_activity_by_entity(pool, user_id, "plan_task", None, 200).await?;

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
    plan_detail::list_links(pool, user_id, id).await
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
    plan_detail::add_link(pool, user_id, id, system, &body.url, body.label.as_deref())
        .await?
        .ok_or_else(|| AppError::NotFound("Plan".into()))
}

/// Mirrors Node's `DELETE /plans/:id/links/:linkId` (`packages/api/src/plans/routes.ts:216-224`).
pub async fn remove_link(pool: &PgPool, user_id: &str, id: &str, link_id: &str) -> AppResult<()> {
    get_plan(pool, user_id, id).await?;
    if plan_detail::remove_link(pool, user_id, id, link_id).await? {
        Ok(())
    } else {
        Err(AppError::NotFound("PlanExternalLink".into()))
    }
}
