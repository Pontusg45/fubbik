use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

fn dev_state(pool: sqlx::PgPool) -> fubbik_api::AppState {
    fubbik_api::AppState {
        pool,
        implicit_dev_session: true,
    }
}

async fn seed_dev_user(pool: &sqlx::PgPool) {
    fubbik_db::repo::user::create(pool, "dev@localhost", "Dev", None)
        .await
        .unwrap();
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn create_then_fetch_chunk(pool: sqlx::PgPool) {
    seed_dev_user(&pool).await;
    let app = fubbik_api::router(dev_state(pool));

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
    assert_eq!(res.status(), StatusCode::OK);

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
    seed_dev_user(&pool).await;
    let app = fubbik_api::router(dev_state(pool));

    let res = app
        .oneshot(
            Request::get("/api/chunks/nonexistent")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn unauthenticated_request_is_401(pool: sqlx::PgPool) {
    let app = fubbik_api::router(fubbik_api::AppState {
        pool,
        implicit_dev_session: false,
    });

    let res = app
        .oneshot(Request::get("/api/chunks").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn blank_title_is_400(pool: sqlx::PgPool) {
    seed_dev_user(&pool).await;
    let app = fubbik_api::router(dev_state(pool));

    let res = app
        .oneshot(
            Request::post("/api/chunks")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"title":"   "}"#))
                .unwrap(),
        )
        .await
        .unwrap();
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
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice::<serde_json::Value>(&body).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string()
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn blank_title_on_update_is_400(pool: sqlx::PgPool) {
    seed_dev_user(&pool).await;
    let app = fubbik_api::router(dev_state(pool));
    let id = create_chunk(&app, "Original").await;

    let res = app
        .oneshot(
            Request::patch(format!("/api/chunks/{id}"))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"title":""}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn whitespace_only_title_on_update_is_400(pool: sqlx::PgPool) {
    seed_dev_user(&pool).await;
    let app = fubbik_api::router(dev_state(pool));
    let id = create_chunk(&app, "Original").await;

    let res = app
        .oneshot(
            Request::patch(format!("/api/chunks/{id}"))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"title":"   "}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn over_length_title_on_update_is_400(pool: sqlx::PgPool) {
    seed_dev_user(&pool).await;
    let app = fubbik_api::router(dev_state(pool));
    let id = create_chunk(&app, "Original").await;

    let too_long = "x".repeat(201);
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
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn update_records_history(pool: sqlx::PgPool) {
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

    let res = app
        .oneshot(
            Request::get("/api/chunks?limit=2&offset=0")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
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
    seed_dev_user(&pool).await;
    let app = fubbik_api::router(dev_state(pool));

    let res = app
        .oneshot(
            Request::get("/api/chunks?limit=999999&offset=-5")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body = res.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        json["limit"], 500,
        "limit must be clamped to the max of 500"
    );
    assert_eq!(json["offset"], 0, "offset must not go negative");
}

/// A freshly created chunk must expose every field Node returns, with
/// Node's null/default semantics preserved: `aliases`/`notAbout` default to
/// `[]` (NOT NULL columns), `alternatives`/`embedding` are `null` when
/// unset (nullable columns), and `scope` defaults to `{}`.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn created_chunk_exposes_all_fields_with_nodes_null_semantics(pool: sqlx::PgPool) {
    seed_dev_user(&pool).await;
    let app = fubbik_api::router(dev_state(pool));

    let res = app
        .oneshot(
            Request::post("/api/chunks")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"title":"Full shape","content":"body"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

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

    let res = app
        .oneshot(
            Request::get(format!("/api/chunks/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let chunk: serde_json::Value = serde_json::from_slice(&body).unwrap();

    let embedding = chunk["embedding"]
        .as_array()
        .expect("embedding must serialise as a JSON array, not a string or null");
    assert_eq!(embedding.len(), 768);
    assert!((embedding[1].as_f64().unwrap() - 0.001).abs() < 1e-6);
}

/// Task 7 added `tags`/`after`/`enrichment`/`minConnections` query params to
/// `GET /api/chunks` as a side effect of building `ListParams` for the
/// `collections` domain. This is the regression guard: with none of the
/// four present, behaviour must be byte-identical to before — same rows,
/// same envelope shape.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn list_is_unchanged_when_the_new_filter_params_are_absent(pool: sqlx::PgPool) {
    seed_dev_user(&pool).await;
    let app = fubbik_api::router(dev_state(pool));

    for i in 0..3 {
        create_chunk(&app, &format!("T{i}")).await;
    }

    let res = app
        .oneshot(Request::get("/api/chunks").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["chunks"].as_array().unwrap().len(), 3);
    assert_eq!(json["total"], 3);
    assert_eq!(json["limit"], 50);
    assert_eq!(json["offset"], 0);
}

/// `tags`/`enrichment`/`minConnections`/`after` reach `ListParams` from the
/// query string. `enrichment` only exercises the strict-enum path
/// (`?enrichment=bogus` -> 400, same already-accepted divergence as
/// `?sort=bogus` — see `ListChunksQuery`); `tags` proves the OR-semantics
/// comma-split reaches the repository end to end.
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn list_accepts_the_new_query_params(pool: sqlx::PgPool) {
    seed_dev_user(&pool).await;
    let app = fubbik_api::router(dev_state(pool.clone()));

    let id = create_chunk(&app, "Tagged").await;
    let dev_id: String =
        sqlx::query_scalar!(r#"SELECT id FROM "user" WHERE email = $1"#, "dev@localhost")
            .fetch_one(&pool)
            .await
            .unwrap();
    let tag = fubbik_db::repo::tag::create(&pool, &dev_id, "important", None)
        .await
        .unwrap();
    fubbik_db::repo::tag::set_chunk_tags(&pool, &dev_id, &id, &[tag.id])
        .await
        .unwrap();

    let res = app
        .clone()
        .oneshot(
            Request::get("/api/chunks?tags=important&after=30&minConnections=0")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let chunks = json["chunks"].as_array().unwrap();
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0]["id"], id);

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
