use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

fn dev_state(pool: sqlx::PgPool) -> fubbik_api::AppState {
    fubbik_api::AppState {
        pool,
        implicit_dev_session: true,
        better_auth_secret: "test-secret".into(),
        ai: fubbik_ai::OllamaClient::new("http://127.0.0.1:1"),
        rate_limiter: Default::default(),
        background: Default::default(),
    }
}

async fn seed_dev_user(pool: &sqlx::PgPool) {
    // The canonical bootstrap, not a bare `user::create`: the implicit-dev
    // fallback now looks the row up by the fixed `id = "dev-user"`
    // (matching Node's `IMPLICIT_DEV_USER_ID`), so a same-email row under
    // an arbitrary id is no longer an equivalent fixture.
    fubbik_db::repo::user::ensure_implicit_dev_user(pool)
        .await
        .unwrap();
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn create_then_fetch_chunk(pool: sqlx::PgPool) {
    // Given
    seed_dev_user(&pool).await;
    let app = fubbik_api::router(dev_state(pool));

    // When
    let res = app
        .clone()
        .oneshot(
            Request::post("/api/chunks")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"title":"Naming","content":"kebab-case"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    // Then
    assert_eq!(res.status(), StatusCode::CREATED);

    let body = res.into_body().collect().await.unwrap().to_bytes();
    let created: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let id = created["id"].as_str().unwrap();
    assert_eq!(created["type"], "note", "type must default to note");

    let res = app
        .oneshot(
            Request::get(format!("/api/chunks/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn missing_chunk_is_404(pool: sqlx::PgPool) {
    // Given
    seed_dev_user(&pool).await;
    let app = fubbik_api::router(dev_state(pool));

    // When
    let res = app
        .oneshot(
            Request::get("/api/chunks/nonexistent")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    // Then
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn unauthenticated_request_is_401(pool: sqlx::PgPool) {
    // Given
    let app = fubbik_api::router(fubbik_api::AppState {
        pool,
        implicit_dev_session: false,
        better_auth_secret: "test-secret".into(),
        ai: fubbik_ai::OllamaClient::new("http://127.0.0.1:1"),
        rate_limiter: Default::default(),
        background: Default::default(),
    });

    // When
    let res = app
        .oneshot(Request::get("/api/chunks").body(Body::empty()).unwrap())
        .await
        .unwrap();
    // Then
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn blank_title_is_400(pool: sqlx::PgPool) {
    // Given
    seed_dev_user(&pool).await;
    let app = fubbik_api::router(dev_state(pool));

    // When
    let res = app
        .oneshot(
            Request::post("/api/chunks")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"title":"   "}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    // Then
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

/// Creates a chunk and returns its id, for tests that need an existing
/// chunk to PATCH.
async fn create_chunk(app: &axum::Router, title: &str) -> String {
    let res = app
        .clone()
        .oneshot(
            Request::post("/api/chunks")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({ "title": title, "content": "body" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice::<serde_json::Value>(&body).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string()
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn blank_title_on_update_is_400(pool: sqlx::PgPool) {
    // Given
    seed_dev_user(&pool).await;
    let app = fubbik_api::router(dev_state(pool));
    let id = create_chunk(&app, "Original").await;

    // When
    let res = app
        .oneshot(
            Request::patch(format!("/api/chunks/{id}"))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"title":""}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    // Then
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn whitespace_only_title_on_update_is_400(pool: sqlx::PgPool) {
    // Given
    seed_dev_user(&pool).await;
    let app = fubbik_api::router(dev_state(pool));
    let id = create_chunk(&app, "Original").await;

    // When
    let res = app
        .oneshot(
            Request::patch(format!("/api/chunks/{id}"))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"title":"   "}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    // Then
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn over_length_title_on_update_is_400(pool: sqlx::PgPool) {
    // Given
    seed_dev_user(&pool).await;
    let app = fubbik_api::router(dev_state(pool));
    let id = create_chunk(&app, "Original").await;

    let too_long = "x".repeat(201);
    // When
    let res = app
        .oneshot(
            Request::patch(format!("/api/chunks/{id}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({ "title": too_long }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    // Then
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn update_records_history(pool: sqlx::PgPool) {
    // Given
    seed_dev_user(&pool).await;
    let app = fubbik_api::router(dev_state(pool));

    let res = app
        .clone()
        .oneshot(
            Request::post("/api/chunks")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"title":"V1","content":"first"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let id = serde_json::from_slice::<serde_json::Value>(&body).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    app.clone()
        .oneshot(
            Request::patch(format!("/api/chunks/{id}"))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"title":"V2"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    // When
    let res = app
        .oneshot(
            Request::get(format!("/api/chunks/{id}/history"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let history: serde_json::Value = serde_json::from_slice(&body).unwrap();
    // Then
    assert_eq!(history.as_array().unwrap().len(), 1);
    assert_eq!(
        history[0]["title"], "V1",
        "history stores the pre-edit title"
    );
}

/// `GET /api/chunks` must return `{ chunks, total, limit, offset }`, matching
/// the Node/Elysia backend — the web app reads `.chunks` and `.total`
/// directly, and `total` must reflect every matching row, not just the
/// current page.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn list_returns_envelope_with_uncapped_total(pool: sqlx::PgPool) {
    // Given
    seed_dev_user(&pool).await;
    let app = fubbik_api::router(dev_state(pool));

    for i in 0..3 {
        app.clone()
            .oneshot(
                Request::post("/api/chunks")
                    .header("content-type", "application/json")
                    .body(Body::from(format!(r#"{{"title":"T{i}","content":""}}"#)))
                    .unwrap(),
            )
            .await
            .unwrap();
    }

    // When
    let res = app
        .oneshot(
            Request::get("/api/chunks?limit=2&offset=0")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    // Then
    assert_eq!(res.status(), StatusCode::OK);

    let body = res.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        json["chunks"].as_array().unwrap().len(),
        2,
        "page respects limit"
    );
    assert_eq!(
        json["total"], 3,
        "total must count every matching row, not just the current page"
    );
    assert_eq!(json["limit"], 2);
    assert_eq!(json["offset"], 0);
}

/// Out-of-range `limit`/`offset` values are clamped for the actual query;
/// the envelope must echo those clamped values, not the raw request input,
/// or a client reading `limit`/`offset` back would compute the wrong next
/// page.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn list_envelope_echoes_clamped_limit(pool: sqlx::PgPool) {
    // Given
    seed_dev_user(&pool).await;
    let app = fubbik_api::router(dev_state(pool));

    // When
    let res = app
        .oneshot(
            Request::get("/api/chunks?limit=999999&offset=-5")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    // Then
    assert_eq!(res.status(), StatusCode::OK);

    let body = res.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        json["limit"], 100,
        "limit must be clamped to the max of 100, matching Node's \
         `Math.min(Number(query.limit ?? 50), 100)` (packages/api/src/chunks/service.ts:50)"
    );
    assert_eq!(json["offset"], 0, "offset must not go negative");
}

/// A freshly created chunk must expose every field Node returns, with
/// Node's null/default semantics preserved: `aliases`/`notAbout` default to
/// `[]` (NOT NULL columns), `alternatives`/`embedding` are `null` when
/// unset (nullable columns), and `scope` defaults to `{}`.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn created_chunk_exposes_all_fields_with_nodes_null_semantics(pool: sqlx::PgPool) {
    // Given
    seed_dev_user(&pool).await;
    let app = fubbik_api::router(dev_state(pool));

    // When
    let res = app
        .oneshot(
            Request::post("/api/chunks")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"title":"Full shape","content":"body"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    // Then
    assert_eq!(res.status(), StatusCode::CREATED);

    let body = res.into_body().collect().await.unwrap().to_bytes();
    let chunk: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(chunk["aliases"], serde_json::json!([]));
    assert_eq!(chunk["notAbout"], serde_json::json!([]));
    assert_eq!(chunk["scope"], serde_json::json!({}));
    assert_eq!(chunk["alternatives"], serde_json::Value::Null);
    assert_eq!(chunk["embedding"], serde_json::Value::Null);
    assert_eq!(chunk["embeddingUpdatedAt"], serde_json::Value::Null);
    assert_eq!(chunk["isEntryPoint"], false);
    assert_eq!(chunk["reviewedAt"], serde_json::Value::Null);
    assert_eq!(chunk["reviewedBy"], serde_json::Value::Null);
    assert_eq!(chunk["documentId"], serde_json::Value::Null);
    assert_eq!(chunk["documentOrder"], serde_json::Value::Null);
}

/// A populated `embedding` column must round-trip as a plain JSON array of
/// numbers on the wire — the same shape Node's `vector` custom type
/// produces — not as a string or the raw pgvector text form.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn populated_embedding_round_trips_as_a_json_number_array(pool: sqlx::PgPool) {
    // Given
    seed_dev_user(&pool).await;
    let app = fubbik_api::router(dev_state(pool.clone()));

    let res = app
        .clone()
        .oneshot(
            Request::post("/api/chunks")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"title":"Embedded","content":"body"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let id = serde_json::from_slice::<serde_json::Value>(&body).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    // The repository doesn't yet expose a write path for `embedding`
    // (enrichment lands in a later phase) — set it directly to prove the
    // read path decodes a populated vector correctly.
    let vector_literal = format!(
        "[{}]",
        (0..768)
            .map(|i| format!("{:.4}", i as f32 / 1000.0))
            .collect::<Vec<_>>()
            .join(",")
    );
    sqlx::query("UPDATE chunk SET embedding = $1::vector WHERE id = $2")
        .bind(&vector_literal)
        .bind(&id)
        .execute(&pool)
        .await
        .unwrap();

    // When
    let res = app
        .oneshot(
            Request::get(format!("/api/chunks/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let detail: serde_json::Value = serde_json::from_slice(&body).unwrap();

    // `GET /api/chunks/{id}` returns the enriched detail envelope, so the
    // row itself lives under `chunk` — see `chunks::dto::ChunkDetail` and
    // `tests/chunk_detail.rs`.
    let embedding = detail["chunk"]["embedding"]
        .as_array()
        .expect("embedding must serialise as a JSON array, not a string or null");
    // Then
    assert_eq!(embedding.len(), 768);
    assert!((embedding[1].as_f64().unwrap() - 0.001).abs() < 1e-6);
}

/// Task 7 added `tags`/`after`/`enrichment`/`minConnections`/`spaceId` query
/// params to `GET /api/chunks` as a side effect of building `ListParams` for
/// the `collections` domain (`spaceId` arrived slightly later than the other
/// four, once `collections::service::get_chunks` needed to thread
/// `collection.spaceId` through — see `task-7-report.md`). This is the
/// regression guard: with none of the five present, behaviour must be
/// byte-identical to before — same rows, same envelope shape.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn list_is_unchanged_when_the_new_filter_params_are_absent(pool: sqlx::PgPool) {
    // Given
    seed_dev_user(&pool).await;
    let app = fubbik_api::router(dev_state(pool));

    for i in 0..3 {
        create_chunk(&app, &format!("T{i}")).await;
    }

    // When
    let res = app
        .oneshot(Request::get("/api/chunks").body(Body::empty()).unwrap())
        .await
        .unwrap();
    // Then
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["chunks"].as_array().unwrap().len(), 3);
    assert_eq!(json["total"], 3);
    assert_eq!(json["limit"], 50);
    assert_eq!(json["offset"], 0);
}

/// `spaceId` reaches `ListParams` from the query string and narrows results
/// to the named space plus global (no-space) chunks — see
/// `chunk::ListParams::space_id`'s doc comment for the exact "or has no
/// space at all" semantics this mirrors from Node.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn list_accepts_space_id_query_param(pool: sqlx::PgPool) {
    // Given
    seed_dev_user(&pool).await;
    let app = fubbik_api::router(dev_state(pool.clone()));
    let dev_id: String =
        sqlx::query_scalar!(r#"SELECT id FROM "user" WHERE email = $1"#, "dev@localhost")
            .fetch_one(&pool)
            .await
            .unwrap();

    let in_space = create_chunk(&app, "In space").await;
    let elsewhere = create_chunk(&app, "Elsewhere").await;
    let space_id = fubbik_db::repo::space::create(
        &pool,
        &dev_id,
        fubbik_db::repo::space::NewSpace {
            name: "a-space".into(),
            kind: "wiki".into(),
            description: None,
        },
        None,
    )
    .await
    .unwrap()
    .id;
    let other_space_id = fubbik_db::repo::space::create(
        &pool,
        &dev_id,
        fubbik_db::repo::space::NewSpace {
            name: "other-space".into(),
            kind: "wiki".into(),
            description: None,
        },
        None,
    )
    .await
    .unwrap()
    .id;
    fubbik_db::repo::space::set_chunk_spaces(
        &pool,
        &dev_id,
        &in_space,
        std::slice::from_ref(&space_id),
    )
    .await
    .unwrap();
    // Assigned to a DIFFERENT space, not left global — Node's `spaceId`
    // filter also matches chunks with no space at all, so leaving this one
    // unassigned would prove nothing about exclusion.
    fubbik_db::repo::space::set_chunk_spaces(
        &pool,
        &dev_id,
        &elsewhere,
        std::slice::from_ref(&other_space_id),
    )
    .await
    .unwrap();

    // When
    let res = app
        .oneshot(
            Request::get(format!("/api/chunks?spaceId={space_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    // Then
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let chunks = json["chunks"].as_array().unwrap();
    assert_eq!(
        chunks.len(),
        1,
        "must exclude the chunk in a different space"
    );
    assert_eq!(chunks[0]["id"], in_space);
}

/// `tags`/`after`/`minConnections` reach `ListParams` from the query
/// string. Each filter gets its own row that MUST be excluded — the
/// original version of this test seeded exactly one chunk and applied all
/// three filters to it at once, which would still have passed with any (or
/// all) of the three params entirely unwired. Follows the same
/// one-row-must-be-excluded shape as `list_accepts_space_id_query_param`
/// above.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn list_accepts_the_new_query_params(pool: sqlx::PgPool) {
    // Given
    seed_dev_user(&pool).await;
    let app = fubbik_api::router(dev_state(pool.clone()));
    let dev_id: String =
        sqlx::query_scalar!(r#"SELECT id FROM "user" WHERE email = $1"#, "dev@localhost")
            .fetch_one(&pool)
            .await
            .unwrap();

    // `tags`: OR-semantics comma-split must exclude a chunk with no
    // matching tag at all.
    let tagged = create_chunk(&app, "Tagged").await;
    let _untagged = create_chunk(&app, "Untagged").await;
    let tag = fubbik_db::repo::tag::create(&pool, &dev_id, "important", None)
        .await
        .unwrap();
    fubbik_db::repo::tag::set_chunk_tags(&pool, &dev_id, &tagged, &[tag.id])
        .await
        .unwrap();

    // When
    let res = app
        .clone()
        .oneshot(
            Request::get("/api/chunks?tags=important")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    // Then
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let chunks = json["chunks"].as_array().unwrap();
    assert_eq!(chunks.len(), 1, "tags must exclude the untagged chunk");
    assert_eq!(chunks[0]["id"], tagged);

    // `after`: a days-ago cutoff must exclude a chunk updated outside the
    // window.
    let recent = create_chunk(&app, "Recent").await;
    let stale = create_chunk(&app, "Stale").await;
    sqlx::query!(
        "UPDATE chunk SET updated_at = now() - interval '90 days' WHERE id = $1",
        stale
    )
    .execute(&pool)
    .await
    .unwrap();

    let res = app
        .clone()
        .oneshot(
            Request::get("/api/chunks?after=30")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let ids: Vec<&str> = json["chunks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap())
        .collect();
    assert!(
        ids.contains(&recent.as_str()),
        "after must keep a chunk updated inside the window"
    );
    assert!(
        !ids.contains(&stale.as_str()),
        "after must exclude a chunk updated outside the window"
    );

    // `minConnections`: must exclude a chunk with fewer connections than
    // the threshold. `?minConnections=0` (the original test's value) is not
    // a real test of this filter at all — the repository skips the
    // connection-count subquery entirely when the threshold is 0 (see
    // `chunk::push_filters`), so it can never exclude anything.
    let connected = create_chunk(&app, "Connected").await;
    let lonely = create_chunk(&app, "Lonely").await;
    let other = create_chunk(&app, "Other").await;
    fubbik_db::repo::connection::create(
        &pool,
        &fubbik_db::new_id(),
        &dev_id,
        &connected,
        &other,
        "related_to",
        "human",
        "approved",
    )
    .await
    .unwrap()
    .expect("own chunks must be linkable");

    let res = app
        .clone()
        .oneshot(
            Request::get("/api/chunks?minConnections=1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let ids: Vec<&str> = json["chunks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap())
        .collect();
    assert!(
        ids.contains(&connected.as_str()),
        "minConnections must keep a chunk meeting the threshold"
    );
    assert!(
        !ids.contains(&lonely.as_str()),
        "minConnections must exclude a chunk below the threshold"
    );

    let res = app
        .oneshot(
            Request::get("/api/chunks?enrichment=bogus")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        res.status(),
        StatusCode::BAD_REQUEST,
        "an unrecognised enrichment value is a 400, same divergence as `sort`"
    );
}
