use fubbik_db::repo::{coordination, plan, user};

async fn board(pool: &sqlx::PgPool, suffix: &str) -> (String, String, String) {
    let user_id = user::create(pool, &format!("{suffix}@coord.test"), "Agent User", None)
        .await
        .unwrap()
        .id;
    let plan = plan::create(pool, &user_id, "Coordinate work", None, None)
        .await
        .unwrap();
    let task = plan::create_task(
        pool,
        &user_id,
        &plan.id,
        "Research",
        None,
        serde_json::json!([]),
        None,
    )
    .await
    .unwrap()
    .unwrap();
    (user_id, plan.id, task.id)
}

async fn run(
    pool: &sqlx::PgPool,
    user_id: &str,
    plan_id: &str,
    handle: &str,
) -> coordination::AgentRun {
    coordination::join_run(
        pool,
        user_id,
        plan_id,
        coordination::JoinRun {
            handle: handle.into(),
            parent_run_id: None,
            external_key: Some(format!("test/{handle}")),
            capabilities: vec![],
            metadata: serde_json::json!({}),
        },
    )
    .await
    .unwrap()
}

#[sqlx::test]
async fn run_reconnect_and_journal_sync_are_durable(pool: sqlx::PgPool) {
    // Given
    let (user_id, plan_id, task_id) = board(&pool, "reconnect").await;
    let root = coordination::join_run(
        &pool,
        &user_id,
        &plan_id,
        coordination::JoinRun {
            handle: "root".into(),
            parent_run_id: None,
            external_key: Some("thread/root".into()),
            capabilities: vec!["delegate".into()],
            metadata: serde_json::json!({}),
        },
    )
    .await
    .unwrap();
    // When
    let reconnected = coordination::join_run(
        &pool,
        &user_id,
        &plan_id,
        coordination::JoinRun {
            handle: "root".into(),
            parent_run_id: None,
            external_key: Some("thread/root".into()),
            capabilities: vec!["delegate".into()],
            metadata: serde_json::json!({}),
        },
    )
    .await
    .unwrap();
    // Then
    assert_eq!(root.id, reconnected.id);

    let entry = coordination::append_entry(
        &pool,
        &user_id,
        &plan_id,
        coordination::NewEntry {
            author_run_id: root.id.clone(),
            recipient_run_id: None,
            task_id: Some(task_id),
            reply_to_id: None,
            kind: "note".into(),
            body: "Persistent context".into(),
            metadata: serde_json::json!({}),
            client_mutation_id: "note-1".into(),
        },
    )
    .await
    .unwrap();

    let entries =
        coordination::list_entries_after(&pool, &user_id, &plan_id, Some(&root.id), 0, 100)
            .await
            .unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].id, entry.id);
    assert_eq!(entries[0].body, "Persistent context");
}

#[sqlx::test]
async fn direct_entries_are_private_to_author_and_recipient(pool: sqlx::PgPool) {
    // Given
    let (user_id, plan_id, _) = board(&pool, "private").await;
    let root = run(&pool, &user_id, &plan_id, "root").await;
    let child = run(&pool, &user_id, &plan_id, "child").await;
    let sibling = run(&pool, &user_id, &plan_id, "sibling").await;
    // When
    coordination::append_entry(
        &pool,
        &user_id,
        &plan_id,
        coordination::NewEntry {
            task_id: None,
            author_run_id: child.id.clone(),
            recipient_run_id: Some(root.id.clone()),
            reply_to_id: None,
            kind: "question".into(),
            body: "secret".into(),
            metadata: serde_json::json!({}),
            client_mutation_id: "secret-1".into(),
        },
    )
    .await
    .unwrap();

    // Then
    assert_eq!(
        coordination::list_entries_after(&pool, &user_id, &plan_id, Some(&root.id), 0, 10)
            .await
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        coordination::list_entries_after(&pool, &user_id, &plan_id, Some(&child.id), 0, 10)
            .await
            .unwrap()
            .len(),
        1
    );
    assert!(
        coordination::list_entries_after(&pool, &user_id, &plan_id, Some(&sibling.id), 0, 10)
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        coordination::list_entries_after(&pool, &user_id, &plan_id, None, 0, 10)
            .await
            .unwrap()
            .len(),
        1
    );
}

#[sqlx::test]
async fn concurrent_entry_retry_creates_one_row(pool: sqlx::PgPool) {
    // Given
    let (user_id, plan_id, _) = board(&pool, "entry-retry").await;
    let root = run(&pool, &user_id, &plan_id, "root").await;
    let input = || coordination::NewEntry {
        task_id: None,
        author_run_id: root.id.clone(),
        recipient_run_id: None,
        reply_to_id: None,
        kind: "note".into(),
        body: "retry me".into(),
        metadata: serde_json::json!({}),
        client_mutation_id: "same-mutation".into(),
    };
    let (a, b) = tokio::join!(
        coordination::append_entry(&pool, &user_id, &plan_id, input()),
        coordination::append_entry(&pool, &user_id, &plan_id, input())
    );
    let a = a.unwrap();
    // When
    let b = b.unwrap();
    // Then
    assert_eq!(a.id, b.id);
    assert_eq!(
        coordination::list_entries_after(&pool, &user_id, &plan_id, None, 0, 10)
            .await
            .unwrap()
            .len(),
        1
    );
}

#[sqlx::test]
async fn one_agent_wins_a_concurrent_claim_and_expiry_allows_takeover(pool: sqlx::PgPool) {
    // Given
    let (user_id, plan_id, task_id) = board(&pool, "claim").await;
    let a = run(&pool, &user_id, &plan_id, "a").await;
    let b = run(&pool, &user_id, &plan_id, "b").await;
    let pa = pool.clone();
    let pb = pool.clone();
    let ua = user_id.clone();
    let ub = user_id.clone();
    let pla = plan_id.clone();
    let plb = plan_id.clone();
    let ta = task_id.clone();
    let tb = task_id.clone();
    let ra = a.id.clone();
    let rb = b.id.clone();
    // When
    let (left, right) = tokio::join!(
        coordination::claim_task(&pa, &ua, &pla, &ta, &ra, 600),
        coordination::claim_task(&pb, &ub, &plb, &tb, &rb, 600)
    );
    // Then
    assert_ne!(
        left.is_ok(),
        right.is_ok(),
        "exactly one concurrent claim must win"
    );

    sqlx::query("UPDATE plan_task_claim SET lease_expires_at = now() - interval '1 second' WHERE task_id=$1")
        .bind(&task_id)
        .execute(&pool)
        .await
        .unwrap();
    let previous = left
        .ok()
        .map(|c| c.agent_run_id)
        .or_else(|| right.ok().map(|c| c.agent_run_id))
        .unwrap();
    let takeover = if previous == a.id { &b.id } else { &a.id };
    let claim = coordination::claim_task(&pool, &user_id, &plan_id, &task_id, takeover, 600)
        .await
        .unwrap();
    assert_eq!(&claim.agent_run_id, takeover);
}

#[sqlx::test]
async fn renew_never_acquires_an_absent_or_expired_claim(pool: sqlx::PgPool) {
    // Given
    let (user_id, plan_id, task_id) = board(&pool, "renew").await;
    // When
    let worker = run(&pool, &user_id, &plan_id, "worker").await;
    // Then
    assert!(
        coordination::renew_task(&pool, &user_id, &plan_id, &task_id, &worker.id, 600)
            .await
            .is_err()
    );
    coordination::claim_task(&pool, &user_id, &plan_id, &task_id, &worker.id, 600)
        .await
        .unwrap();
    coordination::renew_task(&pool, &user_id, &plan_id, &task_id, &worker.id, 600)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE plan_task_claim SET lease_expires_at=now()-interval '1 second' WHERE task_id=$1",
    )
    .bind(&task_id)
    .execute(&pool)
    .await
    .unwrap();
    assert!(
        coordination::renew_task(&pool, &user_id, &plan_id, &task_id, &worker.id, 600)
            .await
            .is_err()
    );
}

#[sqlx::test]
async fn terminal_transition_is_atomic_releases_claim_and_is_retry_safe(pool: sqlx::PgPool) {
    // Given
    let (user_id, plan_id, task_id) = board(&pool, "transition").await;
    let child = run(&pool, &user_id, &plan_id, "worker").await;
    coordination::claim_task(&pool, &user_id, &plan_id, &task_id, &child.id, 600)
        .await
        .unwrap();

    // When
    let first = coordination::transition_claimed_task(
        &pool,
        &user_id,
        &plan_id,
        &task_id,
        &child.id,
        "done",
        Some("Finished"),
        "transition-1",
    )
    .await
    .unwrap();
    // Then
    assert_eq!(first.0.status, "done");
    assert!(
        coordination::list_claims(&pool, &user_id, &plan_id)
            .await
            .unwrap()
            .is_empty()
    );

    let replay = coordination::transition_claimed_task(
        &pool,
        &user_id,
        &plan_id,
        &task_id,
        &child.id,
        "done",
        Some("Finished"),
        "transition-1",
    )
    .await
    .unwrap();
    assert_eq!(first.1.id, replay.1.id);
    assert_eq!(
        coordination::list_entries_after(&pool, &user_id, &plan_id, None, 0, 10)
            .await
            .unwrap()
            .len(),
        1
    );
}

#[sqlx::test]
async fn concurrent_transition_retry_returns_the_same_result(pool: sqlx::PgPool) {
    // Given
    let (user_id, plan_id, task_id) = board(&pool, "transition-race").await;
    let child = run(&pool, &user_id, &plan_id, "worker").await;
    coordination::claim_task(&pool, &user_id, &plan_id, &task_id, &child.id, 600)
        .await
        .unwrap();

    let transition = || {
        coordination::transition_claimed_task(
            &pool,
            &user_id,
            &plan_id,
            &task_id,
            &child.id,
            "done",
            Some("Finished concurrently"),
            "transition-race-1",
        )
    };
    let (first, retry) = tokio::join!(transition(), transition());
    let first = first.unwrap();
    // When
    let retry = retry.unwrap();

    // Then
    assert_eq!(first.1.id, retry.1.id);
    assert_eq!(first.0.status, "done");
    assert_eq!(retry.0.status, "done");
    assert_eq!(
        coordination::list_entries_after(&pool, &user_id, &plan_id, None, 0, 10)
            .await
            .unwrap()
            .len(),
        1
    );
}

#[sqlx::test]
async fn acknowledgements_only_move_forward(pool: sqlx::PgPool) {
    // Given
    let (user_id, plan_id, _) = board(&pool, "ack").await;
    let root = run(&pool, &user_id, &plan_id, "root").await;
    let entry = coordination::append_entry(
        &pool,
        &user_id,
        &plan_id,
        coordination::NewEntry {
            task_id: None,
            author_run_id: root.id.clone(),
            recipient_run_id: None,
            reply_to_id: None,
            kind: "note".into(),
            body: "one".into(),
            metadata: serde_json::json!({}),
            client_mutation_id: "ack-note".into(),
        },
    )
    .await
    .unwrap();
    let advanced = coordination::ack_run(&pool, &user_id, &plan_id, &root.id, entry.sequence, None)
        .await
        .unwrap();
    // When
    let regressed = coordination::ack_run(&pool, &user_id, &plan_id, &root.id, 0, None)
        .await
        .unwrap();
    // Then
    assert_eq!(advanced.last_ack_sequence, entry.sequence);
    assert_eq!(regressed.last_ack_sequence, entry.sequence);
}

#[sqlx::test]
async fn deleting_a_plan_cascades_a_threaded_board(pool: sqlx::PgPool) {
    // Given
    let (user_id, plan_id, task_id) = board(&pool, "cascade").await;
    let root = run(&pool, &user_id, &plan_id, "root").await;
    let child = coordination::join_run(
        &pool,
        &user_id,
        &plan_id,
        coordination::JoinRun {
            handle: "child".into(),
            parent_run_id: Some(root.id.clone()),
            external_key: Some("cascade/child".into()),
            capabilities: vec![],
            metadata: serde_json::json!({}),
        },
    )
    .await
    .unwrap();
    let first = coordination::append_entry(
        &pool,
        &user_id,
        &plan_id,
        coordination::NewEntry {
            task_id: Some(task_id),
            author_run_id: child.id.clone(),
            recipient_run_id: Some(root.id),
            reply_to_id: None,
            kind: "question".into(),
            body: "Question".into(),
            metadata: serde_json::json!({}),
            client_mutation_id: "thread-1".into(),
        },
    )
    .await
    .unwrap();
    // When
    coordination::append_entry(
        &pool,
        &user_id,
        &plan_id,
        coordination::NewEntry {
            task_id: None,
            author_run_id: child.id,
            recipient_run_id: None,
            reply_to_id: Some(first.id),
            kind: "answer".into(),
            body: "Answer".into(),
            metadata: serde_json::json!({}),
            client_mutation_id: "thread-2".into(),
        },
    )
    .await
    .unwrap();

    // Then
    assert!(plan::delete(&pool, &user_id, &plan_id).await.unwrap());
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM agent_run WHERE plan_id=$1")
        .bind(&plan_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(rows, 0);
}
