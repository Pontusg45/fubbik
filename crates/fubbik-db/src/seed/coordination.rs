use anyhow::{Result, anyhow};
use serde::Serialize;
use sqlx::{Postgres, Transaction};
use std::collections::BTreeMap;

use super::plans::PlanSeed;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize)]
pub struct CoordinationCounts {
    pub runs: usize,
    pub claims: usize,
    pub entries: usize,
}

pub async fn seed_coordination(
    tx: &mut Transaction<'_, Postgres>,
    plan: &PlanSeed,
) -> Result<CoordinationCounts> {
    let runs = [
        (
            "coordinator",
            None,
            "active",
            &["delegate", "review", "synthesize"][..],
            "root",
            90,
            4,
        ),
        (
            "ui-implementer",
            Some("coordinator"),
            "active",
            &["react", "accessibility", "integration-tests"][..],
            "sub-agent",
            72,
            2,
        ),
        (
            "schema-reviewer",
            Some("coordinator"),
            "finished",
            &["postgres", "drizzle", "review"][..],
            "sub-agent",
            80,
            28,
        ),
    ];
    let mut run_ids = BTreeMap::new();
    for (handle, parent, status, capabilities, role, age, heartbeat_age) in runs {
        let external_key = format!("seed/vocab-crud/{handle}");
        let existing: Option<String> =
            sqlx::query_scalar("SELECT id FROM agent_run WHERE plan_id = $1 AND external_key = $2")
                .bind(&plan.plan_id)
                .bind(&external_key)
                .fetch_optional(&mut **tx)
                .await?;
        let id = existing.unwrap_or_else(|| format!("seed-agent-run-{handle}"));
        let parent_id = parent.and_then(|name| run_ids.get(name));
        sqlx::query(
            "INSERT INTO agent_run (id, plan_id, parent_run_id, handle, external_key, status, capabilities, metadata, \
             created_at, updated_at, last_heartbeat_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, \
             now() - $9 * interval '1 minute', now() - $10 * interval '1 minute', \
             now() - $10 * interval '1 minute') ON CONFLICT (id) DO UPDATE SET \
             status = EXCLUDED.status, capabilities = EXCLUDED.capabilities, metadata = EXCLUDED.metadata, \
             last_heartbeat_at = EXCLUDED.last_heartbeat_at, updated_at = EXCLUDED.updated_at",
        )
        .bind(&id).bind(&plan.plan_id).bind(parent_id).bind(handle).bind(&external_key)
        .bind(status).bind(serde_json::json!(capabilities))
        .bind(serde_json::json!({"host":"seed","role":role}))
        .bind(age).bind(heartbeat_age)
        .execute(&mut **tx).await?;
        run_ids.insert(handle, id);
    }
    let implementer = &run_ids["ui-implementer"];
    let inverse_task = &plan.task_ids["inverse-ui"];
    sqlx::query(
        "INSERT INTO plan_task_claim (task_id, plan_id, agent_run_id, claimed_at, lease_expires_at, updated_at) \
         VALUES ($1, $2, $3, now() - interval '15 minutes', now() + interval '45 minutes', \
         now() - interval '2 minutes') ON CONFLICT (task_id) DO UPDATE SET \
         agent_run_id = EXCLUDED.agent_run_id, lease_expires_at = EXCLUDED.lease_expires_at, \
         updated_at = EXCLUDED.updated_at",
    )
    .bind(inverse_task).bind(&plan.plan_id).bind(implementer)
    .execute(&mut **tx).await?;

    let entries = [
        (
            "kickoff",
            "schema",
            "coordinator",
            None,
            None,
            "system",
            "Split the catalog work into schema review and UI implementation tracks.",
            serde_json::json!({"phase":"kickoff"}),
            "seed-coordination-kickoff",
            88,
        ),
        (
            "artifact",
            "fk-cutover",
            "schema-reviewer",
            None,
            None,
            "artifact",
            "Schema review complete: catalog foreign keys and cascade behavior are verified.",
            serde_json::json!({"artifact":"schema-review.md","verdict":"approved"}),
            "seed-schema-review-artifact",
            34,
        ),
        (
            "decision",
            "ui-catalog",
            "coordinator",
            None,
            None,
            "decision",
            "Keep catalog slugs stable and use labels only for presentation.",
            serde_json::json!({"decision":"stable-catalog-slugs"}),
            "seed-catalog-slug-decision",
            31,
        ),
        (
            "handoff",
            "crud-ui",
            "schema-reviewer",
            Some("ui-implementer"),
            None,
            "handoff",
            "The schema path is clear. Continue with inverse labels and preserve the catalog IDs on writes.",
            serde_json::json!({"fromDiscipline":"schema","toDiscipline":"frontend"}),
            "seed-schema-to-ui-handoff",
            27,
        ),
        (
            "progress",
            "inverse-ui",
            "ui-implementer",
            None,
            None,
            "progress",
            "Inverse labels render on chunk detail; integration coverage is the remaining work.",
            serde_json::json!({"percent":75}),
            "seed-inverse-label-progress",
            12,
        ),
        (
            "question",
            "inverse-ui",
            "ui-implementer",
            Some("coordinator"),
            None,
            "question",
            "Should the fallback show the relation slug when an inverse label is absent?",
            serde_json::json!({}),
            "seed-inverse-label-question",
            8,
        ),
        (
            "answer",
            "inverse-ui",
            "coordinator",
            Some("ui-implementer"),
            Some("question"),
            "answer",
            "Yes. Prefer the inverse label, then fall back to a humanized relation slug.",
            serde_json::json!({}),
            "seed-inverse-label-answer",
            5,
        ),
    ];
    let mut entry_ids = BTreeMap::new();
    for (name, task, author, recipient, reply_to, kind, body, metadata, mutation, age) in entries {
        let task_id = plan
            .task_ids
            .get(task)
            .ok_or_else(|| anyhow!("Unknown plan task: {task}"))?;
        let author_id = &run_ids[author];
        let recipient_id = recipient.map(|name| &run_ids[name]);
        let reply_to_id = reply_to.map(|name| &entry_ids[name]);
        let existing: Option<String> = sqlx::query_scalar(
            "SELECT id FROM coordination_entry WHERE author_run_id = $1 AND client_mutation_id = $2",
        )
        .bind(author_id).bind(mutation).fetch_optional(&mut **tx).await?;
        let id = existing.unwrap_or_else(|| format!("seed-coordination-entry-{name}"));
        sqlx::query(
            "INSERT INTO coordination_entry (id, plan_id, task_id, author_run_id, recipient_run_id, reply_to_id, \
             kind, body, metadata, client_mutation_id, created_at) VALUES ($1, $2, $3, $4, $5, $6, \
             $7, $8, $9, $10, now() - $11 * interval '1 minute') ON CONFLICT (id) DO UPDATE SET \
             body = EXCLUDED.body, metadata = EXCLUDED.metadata",
        )
        .bind(&id).bind(&plan.plan_id).bind(task_id).bind(author_id).bind(recipient_id)
        .bind(reply_to_id).bind(kind).bind(body).bind(metadata).bind(mutation).bind(age)
        .execute(&mut **tx).await?;
        entry_ids.insert(name, id);
    }
    Ok(CoordinationCounts {
        runs: 3,
        claims: 1,
        entries: 7,
    })
}
