use fubbik_core::error::AppResult;
use fubbik_db::repo::coordination;
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
    let board = coordination::read_board(
        pool,
        user_id,
        plan_id,
        query.run_id.as_deref(),
        query.after_sequence.unwrap_or(0),
        query.limit.unwrap_or(100),
    )
    .await?;
    let tasks = board
        .tasks
        .into_iter()
        .map(|task| BoardTask {
            depends_on: task.depends_on,
            id: task.id,
            title: task.title,
            description: task.description,
            status: task.status,
            order: task.order,
        })
        .collect();
    Ok(BoardSnapshot {
        plan: BoardPlan {
            id: board.plan.id,
            title: board.plan.title,
            status: board.plan.status,
        },
        tasks,
        runs: board.runs,
        claims: board.claims,
        entries: board.entries,
        cursor: BoardCursor {
            next_sequence: board.next_sequence,
            acknowledged_sequence: board.acknowledged_sequence,
            has_more: board.has_more,
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
