use fubbik_core::error::{AppError, AppResult};
use fubbik_db::repo::{coordination, plan};
use sqlx::PgPool;

use super::dto::{
    AckRunBody, BoardCursor, BoardPlan, BoardQuery, BoardSnapshot, BoardTask, ClaimAction,
    ClaimBody, ClaimResponse, CreateEntryBody, JoinRunBody, TransitionTaskBody,
    TransitionTaskResponse,
};

pub async fn join(
    pool: &PgPool,
    user_id: &str,
    plan_id: &str,
    body: JoinRunBody,
) -> AppResult<coordination::AgentRun> {
    coordination::join_run(
        pool,
        user_id,
        plan_id,
        coordination::JoinRun {
            handle: body.handle,
            parent_run_id: body.parent_run_id,
            external_key: body.external_key,
            capabilities: body.capabilities,
            metadata: body.metadata,
        },
    )
    .await
}

pub async fn board(
    pool: &PgPool,
    user_id: &str,
    plan_id: &str,
    query: BoardQuery,
) -> AppResult<BoardSnapshot> {
    let after = query.after_sequence.unwrap_or(0);
    if after < 0 {
        return Err(AppError::Validation(
            "afterSequence cannot be negative".into(),
        ));
    }
    let limit = query.limit.unwrap_or(100);
    if limit <= 0 {
        return Err(AppError::Validation("limit must be positive".into()));
    }
    let limit = limit.min(500);
    let found = plan::find_by_id(pool, user_id, plan_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Plan".into()))?;
    let run = match query.run_id.as_deref() {
        Some(id) => Some(
            coordination::find_run(pool, user_id, plan_id, id)
                .await?
                .ok_or_else(|| AppError::NotFound("Agent run".into()))?,
        ),
        None => None,
    };
    let tasks = plan::list_tasks(pool, user_id, plan_id).await?;
    let dependencies = plan::list_task_dependencies(pool, user_id, plan_id).await?;
    let runs = coordination::list_runs(pool, user_id, plan_id).await?;
    let claims = coordination::list_claims(pool, user_id, plan_id).await?;
    let mut entries = coordination::list_entries_after(
        pool,
        user_id,
        plan_id,
        query.run_id.as_deref(),
        after,
        limit + 1,
    )
    .await?;
    let has_more = entries.len() > limit as usize;
    if has_more {
        entries.truncate(limit as usize);
    }
    let global_max = coordination::max_sequence(pool, user_id, plan_id).await?;
    let next_sequence = if has_more {
        entries.last().map(|e| e.sequence).unwrap_or(after)
    } else {
        global_max.max(after)
    };
    let tasks = tasks
        .into_iter()
        .map(|task| BoardTask {
            depends_on: dependencies
                .iter()
                .filter(|d| d.task_id == task.id)
                .map(|d| d.depends_on_task_id.clone())
                .collect(),
            id: task.id,
            title: task.title,
            description: task.description,
            status: task.status,
            order: task.order,
        })
        .collect();
    Ok(BoardSnapshot {
        plan: BoardPlan {
            id: found.id,
            title: found.title,
            status: found.status,
        },
        tasks,
        runs,
        claims,
        entries,
        cursor: BoardCursor {
            next_sequence,
            acknowledged_sequence: run.map(|r| r.last_ack_sequence),
            has_more,
        },
    })
}

pub async fn mutate_claim(
    pool: &PgPool,
    user_id: &str,
    plan_id: &str,
    task_id: &str,
    body: ClaimBody,
) -> AppResult<ClaimResponse> {
    match body.action {
        ClaimAction::Claim => {
            let claim = coordination::claim_task(
                pool,
                user_id,
                plan_id,
                task_id,
                &body.run_id,
                body.lease_seconds.unwrap_or(600),
            )
            .await?;
            Ok(ClaimResponse {
                action: "claim".into(),
                claim: Some(claim),
            })
        }
        ClaimAction::Renew => {
            let claim = coordination::renew_task(
                pool,
                user_id,
                plan_id,
                task_id,
                &body.run_id,
                body.lease_seconds.unwrap_or(600),
            )
            .await?;
            Ok(ClaimResponse {
                action: "renew".into(),
                claim: Some(claim),
            })
        }
        ClaimAction::Release => {
            coordination::release_task(pool, user_id, plan_id, task_id, &body.run_id).await?;
            Ok(ClaimResponse {
                action: "release".into(),
                claim: None,
            })
        }
    }
}

pub async fn transition(
    pool: &PgPool,
    user_id: &str,
    plan_id: &str,
    task_id: &str,
    body: TransitionTaskBody,
) -> AppResult<TransitionTaskResponse> {
    let (task, entry) = coordination::transition_claimed_task(
        pool,
        user_id,
        plan_id,
        task_id,
        &body.run_id,
        &body.status,
        body.note.as_deref(),
        &body.client_mutation_id,
    )
    .await?;
    Ok(TransitionTaskResponse { task, entry })
}

pub async fn write_entry(
    pool: &PgPool,
    user_id: &str,
    plan_id: &str,
    body: CreateEntryBody,
) -> AppResult<coordination::CoordinationEntry> {
    coordination::append_entry(
        pool,
        user_id,
        plan_id,
        coordination::NewEntry {
            task_id: body.task_id,
            author_run_id: body.run_id,
            recipient_run_id: body.recipient_run_id,
            reply_to_id: body.reply_to_id,
            kind: body.kind,
            body: body.body,
            metadata: body.metadata,
            client_mutation_id: body.client_mutation_id,
        },
    )
    .await
}

pub async fn ack(
    pool: &PgPool,
    user_id: &str,
    plan_id: &str,
    run_id: &str,
    body: AckRunBody,
) -> AppResult<coordination::AgentRun> {
    coordination::ack_run(
        pool,
        user_id,
        plan_id,
        run_id,
        body.through_sequence,
        body.status.as_deref(),
    )
    .await
}
