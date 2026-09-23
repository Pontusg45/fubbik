use axum::extract::{Path, State};
use axum::{Json, Router};
use fubbik_db::repo::chunk::Chunk;
use fubbik_db::repo::chunk_group::{self, GroupBy, GroupConstraint, GroupCount, GroupFilters};

use crate::AppState;
use crate::auth::CurrentUser;
use crate::error::ApiResult;
use crate::extract::Query;

#[derive(Debug, Clone, Copy, serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum TagMode {
    Any,
    All,
}

#[derive(Debug, Clone, serde::Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct GroupedQuery {
    pub group_by: String,
    pub tag_type_id: Option<String>,
    pub sub_group_by: Option<String>,
    pub sub_tag_type_id: Option<String>,
    pub codebase_id: Option<String>,
    /// Preferred post-rename alias used by the current web app.
    pub space_id: Option<String>,
    pub workspace_id: Option<String>,
    pub global: Option<String>,
    #[serde(rename = "type")]
    pub chunk_type: Option<String>,
    pub search: Option<String>,
    pub tags: Option<String>,
    pub tag_mode: Option<TagMode>,
    pub origin: Option<String>,
    pub review_status: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct GroupChunksQuery {
    pub group_by: String,
    pub tag_type_id: Option<String>,
    pub codebase_id: Option<String>,
    pub space_id: Option<String>,
    pub workspace_id: Option<String>,
    pub global: Option<String>,
    #[serde(rename = "type")]
    pub chunk_type: Option<String>,
    pub search: Option<String>,
    pub tags: Option<String>,
    pub tag_mode: Option<TagMode>,
    pub origin: Option<String>,
    pub review_status: Option<String>,
    pub sort: Option<String>,
    pub limit: Option<String>,
    pub offset: Option<String>,
}

#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GroupedResponse {
    pub groups: Vec<GroupCount>,
    pub total_groups: usize,
}

#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CompoundGroupCount {
    pub group_name: String,
    pub count: i64,
    pub sub_groups: Vec<GroupCount>,
}

#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CompoundGroupedResponse {
    pub groups: Vec<CompoundGroupCount>,
    pub total_groups: usize,
}

#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
#[serde(untagged)]
pub enum GroupedResult {
    Flat(GroupedResponse),
    Compound(CompoundGroupedResponse),
}

#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
pub struct GroupChunksResponse {
    pub chunks: Vec<Chunk>,
    pub total: i64,
}

fn parse_group_by(raw: &str) -> (GroupBy, Option<String>) {
    match raw {
        "type" => (GroupBy::Type, None),
        "status" => (GroupBy::Status, None),
        "origin" => (GroupBy::Origin, None),
        "freshness" => (GroupBy::Freshness, None),
        "tagtype" => (GroupBy::TagType, None),
        value if value.starts_with("tagtype:") => (
            GroupBy::TagType,
            Some(value.trim_start_matches("tagtype:").to_owned()),
        ),
        _ => (GroupBy::Type, None),
    }
}

fn parse_tags(raw: Option<&str>) -> Vec<String> {
    raw.into_iter()
        .flat_map(|tags| tags.split(','))
        .map(str::trim)
        .filter(|tag| !tag.is_empty())
        .map(str::to_owned)
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn filters(
    user_id: &str,
    chunk_type: Option<String>,
    origin: Option<String>,
    review_status: Option<String>,
    space_id: Option<String>,
    codebase_id: Option<String>,
    workspace_id: Option<String>,
    global: Option<String>,
    tags: Option<String>,
    tag_mode: Option<TagMode>,
) -> GroupFilters {
    GroupFilters {
        user_id: user_id.to_owned(),
        chunk_type,
        origin,
        review_status,
        space_id: space_id.or(codebase_id),
        workspace_id,
        global_only: global.as_deref() == Some("true"),
        tags: parse_tags(tags.as_deref()),
        all_tags: matches!(tag_mode, Some(TagMode::All)),
    }
}

#[utoipa::path(get, path = "/api/chunks/grouped", params(GroupedQuery),
    responses((status = 200, body = GroupedResult)))]
pub async fn grouped(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Query(query): Query<GroupedQuery>,
) -> ApiResult<Json<GroupedResult>> {
    let (group_by, parsed_tag_type) = parse_group_by(&query.group_by);
    let tag_type_id = query.tag_type_id.as_deref().or(parsed_tag_type.as_deref());
    let filters = filters(
        &user.id,
        query.chunk_type,
        query.origin,
        query.review_status,
        query.space_id,
        query.codebase_id,
        query.workspace_id,
        query.global,
        query.tags,
        query.tag_mode,
    );
    let top_groups =
        chunk_group::grouped_counts(&state.pool, &filters, group_by, tag_type_id, None).await?;

    if let Some(raw_sub_group) = query.sub_group_by {
        let (sub_group_by, parsed_sub_tag_type) = parse_group_by(&raw_sub_group);
        let sub_tag_type_id = query
            .sub_tag_type_id
            .as_deref()
            .or(parsed_sub_tag_type.as_deref());
        let mut groups = Vec::with_capacity(top_groups.len());
        for top in top_groups {
            let parent = GroupConstraint {
                group_by,
                group_name: top.group_name.clone(),
                tag_type_id: tag_type_id.map(str::to_owned),
            };
            let sub_groups = chunk_group::grouped_counts(
                &state.pool,
                &filters,
                sub_group_by,
                sub_tag_type_id,
                Some(&parent),
            )
            .await?;
            groups.push(CompoundGroupCount {
                group_name: top.group_name,
                count: top.count,
                sub_groups,
            });
        }
        return Ok(Json(GroupedResult::Compound(CompoundGroupedResponse {
            total_groups: groups.len(),
            groups,
        })));
    }

    Ok(Json(GroupedResult::Flat(GroupedResponse {
        total_groups: top_groups.len(),
        groups: top_groups,
    })))
}

#[utoipa::path(get, path = "/api/chunks/grouped/{groupName}/chunks",
    params(("groupName" = String, Path), GroupChunksQuery),
    responses((status = 200, body = GroupChunksResponse)))]
pub async fn group_chunks(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(group_name): Path<String>,
    Query(query): Query<GroupChunksQuery>,
) -> ApiResult<Json<GroupChunksResponse>> {
    let (group_by, parsed_tag_type) = parse_group_by(&query.group_by);
    let constraint = GroupConstraint {
        group_by,
        group_name,
        tag_type_id: query.tag_type_id.or(parsed_tag_type),
    };
    let filters = filters(
        &user.id,
        query.chunk_type,
        query.origin,
        query.review_status,
        query.space_id,
        query.codebase_id,
        query.workspace_id,
        query.global,
        query.tags,
        query.tag_mode,
    );
    let limit = query
        .limit
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(50)
        .clamp(0, 100);
    let offset = query
        .offset
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(0)
        .max(0);
    let (chunks, total) =
        chunk_group::chunks_in_group(&state.pool, &filters, &constraint, limit, offset).await?;
    Ok(Json(GroupChunksResponse { chunks, total }))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/chunks/grouped", axum::routing::get(grouped))
        .route(
            "/api/chunks/grouped/{groupName}/chunks",
            axum::routing::get(group_chunks),
        )
}

#[cfg(test)]
mod tests {
    use super::{GroupBy, parse_group_by, parse_tags};

    #[test]
    fn parses_inline_tag_type_and_falls_back_to_type() {
        // Given the inline inputs and test fixtures.
        // When the operation is evaluated by the assertion.
        // Then
        assert_eq!(
            parse_group_by("tagtype:domain"),
            (GroupBy::TagType, Some("domain".into()))
        );
        assert_eq!(parse_group_by("unknown"), (GroupBy::Type, None));
    }

    #[test]
    fn normalizes_comma_separated_tags() {
        // Given the inline inputs and test fixtures.
        // When the operation is evaluated by the assertion.
        // Then
        assert_eq!(parse_tags(Some("one, two,,")), vec!["one", "two"]);
    }
}
