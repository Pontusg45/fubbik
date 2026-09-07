use std::collections::HashMap;
use std::time::{Duration, Instant};

use axum::extract::State;
use axum::http::{HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use fubbik_core::error::{AppError, AppResult};

use crate::AppState;
use crate::auth::CurrentUser;
use crate::error::ApiResult;
use crate::extract::Json as ReqJson;

const IMPORT_MAX: u32 = 5;
const IMPORT_WINDOW: Duration = Duration::from_secs(60);

#[derive(Clone, serde::Deserialize, utoipa::ToSchema)]
pub struct ImportDocFile {
    pub path: String,
    pub content: String,
}

#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ImportDocsBody {
    pub files: Vec<ImportDocFile>,
    pub space_id: String,
    pub template_overrides: Option<HashMap<String, Option<String>>>,
}

#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct ImportError {
    pub path: String,
    pub error: String,
}

#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct ImportDocsResult {
    pub created: i32,
    pub skipped: i32,
    pub connections: i32,
    pub errors: Vec<ImportError>,
}

#[derive(serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SuggestedTemplate {
    pub id: String,
    pub name: String,
    pub score: f64,
    #[serde(rename = "type")]
    pub template_type: String,
    pub tags: Vec<String>,
    pub extracted_fields: crate::documents::template_import::ExtractedFields,
}

#[derive(serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PreviewFile {
    pub path: String,
    pub title: String,
    pub suggested_template: Option<SuggestedTemplate>,
    pub parsed: crate::documents::template_import::ParsedDoc,
}

#[derive(serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PreviewResult {
    pub files: Vec<PreviewFile>,
    pub existing_hashes: HashMap<String, String>,
}

async fn verify_request(
    pool: &sqlx::PgPool,
    user_id: &str,
    body: &ImportDocsBody,
) -> AppResult<()> {
    if body.files.len() > 500 {
        return Err(AppError::Validation("at most 500 files are allowed".into()));
    }
    for file in &body.files {
        if file.path.chars().count() > 500 || file.content.chars().count() > 100_000 {
            return Err(AppError::Validation(
                "import file exceeds size limits".into(),
            ));
        }
    }
    fubbik_db::repo::space::find_by_id(pool, user_id, &body.space_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Space".into()))?;
    Ok(())
}

pub async fn preview_service(
    pool: &sqlx::PgPool,
    user_id: &str,
    body: &ImportDocsBody,
) -> AppResult<PreviewResult> {
    verify_request(pool, user_id, body).await?;
    let existing_hashes = fubbik_db::repo::document::list(pool, user_id, Some(&body.space_id))
        .await?
        .into_iter()
        .map(|doc| (doc.source_path, doc.content_hash))
        .collect();
    let templates = fubbik_db::repo::template::list(pool, user_id).await?;
    let mut files = Vec::with_capacity(body.files.len());
    for file in &body.files {
        let parsed = crate::documents::template_import::parse_doc(&file.path, &file.content);
        let suggested_template =
            crate::documents::template_import::best_template(&file.content, &templates).map(
                |(template, score)| {
                    let mappings = template
                        .field_mappings
                        .as_ref()
                        .map_or(&[][..], |m| m.0.as_slice());
                    let (extracted_fields, _) =
                        crate::documents::template_import::extract_fields(&file.content, mappings);
                    let mut tags = template.tags.clone().unwrap_or_default();
                    tags.extend(parsed.tags.clone());
                    let mut seen = std::collections::HashSet::new();
                    tags.retain(|tag| seen.insert(tag.clone()));
                    SuggestedTemplate {
                        id: template.id.clone(),
                        name: template.name.clone(),
                        score,
                        template_type: template.template_type.clone(),
                        tags,
                        extracted_fields,
                    }
                },
            );
        files.push(PreviewFile {
            path: file.path.clone(),
            title: parsed.title.clone(),
            suggested_template,
            parsed,
        });
    }
    Ok(PreviewResult {
        files,
        existing_hashes,
    })
}

async fn import_service(
    pool: &sqlx::PgPool,
    user_id: &str,
    body: &ImportDocsBody,
) -> AppResult<(ImportDocsResult, Vec<(String, String, &'static str, i32)>)> {
    verify_request(pool, user_id, body).await?;
    let mut result = ImportDocsResult {
        created: 0,
        skipped: 0,
        connections: 0,
        errors: Vec::new(),
    };
    let mut file_chunks = HashMap::new();
    let mut events = Vec::new();
    for file in &body.files {
        events.push((file.path.clone(), String::new(), "importing", 0));
        let template_id = body
            .template_overrides
            .as_ref()
            .and_then(|m| m.get(&file.path))
            .and_then(|v| v.as_deref());
        match crate::documents::service::import_document_with_template(
            pool,
            user_id,
            &file.path,
            &file.content,
            Some(&body.space_id),
            template_id,
        )
        .await
        {
            Ok(imported) => {
                if imported.status == crate::documents::dto::ImportStatus::Unchanged {
                    result.skipped += 1;
                    events.push((file.path.clone(), String::new(), "unchanged", 0));
                } else {
                    result.created += imported.created;
                    events.push((
                        file.path.clone(),
                        String::new(),
                        "created",
                        imported.created,
                    ));
                }
                if let Some(id) = imported.first_chunk_id {
                    file_chunks.insert(file.path.clone(), id);
                }
            }
            Err(error) => {
                let message = error.to_string();
                result.errors.push(ImportError {
                    path: file.path.clone(),
                    error: message.clone(),
                });
                events.push((file.path.clone(), message, "error", 0));
            }
        }
    }
    result.connections = create_folder_connections(pool, user_id, &file_chunks)
        .await
        .unwrap_or(0);
    Ok((result, events))
}

async fn create_folder_connections(
    pool: &sqlx::PgPool,
    user_id: &str,
    files: &HashMap<String, String>,
) -> AppResult<i32> {
    let mut by_dir: HashMap<&str, Vec<(&str, &str)>> = HashMap::new();
    for (path, id) in files {
        let dir = path.rsplit_once('/').map_or(".", |(dir, _)| dir);
        by_dir.entry(dir).or_default().push((path, id));
    }
    let mut count = 0;
    for entries in by_dir.values() {
        let Some((_, index_id)) = entries.iter().find(|(path, _)| {
            matches!(
                path.rsplit('/')
                    .next()
                    .unwrap_or(path)
                    .to_lowercase()
                    .as_str(),
                "index.md" | "readme.md" | "_index.md"
            )
        }) else {
            continue;
        };
        for (path, id) in entries {
            let is_index = matches!(
                path.rsplit('/')
                    .next()
                    .unwrap_or(path)
                    .to_lowercase()
                    .as_str(),
                "index.md" | "readme.md" | "_index.md"
            );
            if !is_index
                && fubbik_db::repo::connection::create_if_not_exists(
                    pool, user_id, id, index_id, "part_of",
                )
                .await?
            {
                count += 1;
            }
        }
    }
    Ok(count)
}

#[utoipa::path(post, path = "/api/chunks/import-docs/preview", request_body = ImportDocsBody,
    responses((status = 200, body = PreviewResult)))]
pub async fn preview(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ReqJson(body): ReqJson<ImportDocsBody>,
) -> ApiResult<Json<PreviewResult>> {
    Ok(Json(preview_service(&state.pool, &user.id, &body).await?))
}

fn rate_limit(state: &AppState, user_id: &str) -> Option<Response> {
    let decision =
        state
            .rate_limiter
            .check(&format!("import-docs:{user_id}"), IMPORT_MAX, IMPORT_WINDOW);
    (!decision.allowed).then(|| (StatusCode::TOO_MANY_REQUESTS, Json(serde_json::json!({ "error": "Rate limit exceeded", "retryAfter": decision.retry_after_secs }))).into_response())
}

#[utoipa::path(post, path = "/api/chunks/import-docs", request_body = ImportDocsBody,
    responses((status = 200, body = ImportDocsResult), (status = 429)))]
pub async fn import(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ReqJson(body): ReqJson<ImportDocsBody>,
) -> ApiResult<Response> {
    if let Some(response) = rate_limit(&state, &user.id) {
        return Ok(response);
    }
    let (result, _) = import_service(&state.pool, &user.id, &body).await?;
    Ok(Json(result).into_response())
}

#[utoipa::path(post, path = "/api/chunks/import-docs/stream", request_body = ImportDocsBody,
    responses((status = 200, description = "Server-sent import progress events"), (status = 429)))]
pub async fn stream(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ReqJson(body): ReqJson<ImportDocsBody>,
) -> ApiResult<Response> {
    if let Some(response) = rate_limit(&state, &user.id) {
        return Ok(response);
    }
    verify_request(&state.pool, &user.id, &body).await?;
    let (sender, receiver) =
        tokio::sync::mpsc::channel::<Result<axum::body::Bytes, std::convert::Infallible>>(16);
    let pool = state.pool.clone();
    tokio::spawn(async move {
        let started = Instant::now();
        let (mut created, mut skipped, mut errors) = (0, 0, 0);
        let mut file_chunks = HashMap::new();
        for file in &body.files {
            if send_event(
                &sender,
                "file",
                serde_json::json!({ "type": "file", "path": file.path, "status": "importing" }),
            )
            .await
            .is_err()
            {
                return;
            }
            let template_id = body
                .template_overrides
                .as_ref()
                .and_then(|m| m.get(&file.path))
                .and_then(|v| v.as_deref());
            match crate::documents::service::import_document_with_template(
                &pool,
                &user.id,
                &file.path,
                &file.content,
                Some(&body.space_id),
                template_id,
            )
            .await
            {
                Ok(imported)
                    if imported.status == crate::documents::dto::ImportStatus::Unchanged =>
                {
                    skipped += 1;
                    if let Some(id) = imported.first_chunk_id {
                        file_chunks.insert(file.path.clone(), id);
                    }
                    if send_event(&sender, "file", serde_json::json!({ "type": "file", "path": file.path, "status": "unchanged" })).await.is_err() { return; }
                }
                Ok(imported) => {
                    created += imported.created;
                    if let Some(id) = imported.first_chunk_id {
                        file_chunks.insert(file.path.clone(), id);
                    }
                    if send_event(&sender, "file", serde_json::json!({ "type": "file", "path": file.path, "status": "created", "created": imported.created })).await.is_err() { return; }
                }
                Err(error) => {
                    errors += 1;
                    if send_event(&sender, "file", serde_json::json!({ "type": "file", "path": file.path, "status": "error", "error": error.to_string() })).await.is_err() { return; }
                }
            }
        }
        let connections = create_folder_connections(&pool, &user.id, &file_chunks)
            .await
            .unwrap_or(0);
        let _ = send_event(&sender, "done", serde_json::json!({ "type": "done", "created": created, "skipped": skipped, "errors": errors, "connections": connections, "elapsed": started.elapsed().as_millis() })).await;
    });
    let mut response =
        axum::body::Body::from_stream(tokio_stream::wrappers::ReceiverStream::new(receiver))
            .into_response();
    response.headers_mut().insert(
        "content-type",
        HeaderValue::from_static("text/event-stream"),
    );
    response
        .headers_mut()
        .insert("cache-control", HeaderValue::from_static("no-cache"));
    response
        .headers_mut()
        .insert("connection", HeaderValue::from_static("keep-alive"));
    Ok(response)
}

async fn send_event(
    sender: &tokio::sync::mpsc::Sender<Result<axum::body::Bytes, std::convert::Infallible>>,
    event: &str,
    data: serde_json::Value,
) -> Result<(), ()> {
    sender
        .send(Ok(axum::body::Bytes::from(format!(
            "event: {event}\ndata: {data}\n\n"
        ))))
        .await
        .map_err(|_| ())
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/chunks/import-docs/preview", post(preview))
        .route("/api/chunks/import-docs", post(import))
        .route("/api/chunks/import-docs/stream", post(stream))
}
