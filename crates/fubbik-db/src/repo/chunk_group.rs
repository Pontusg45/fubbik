//! Grouped chunk read model used by the browse UI.
//!
//! This module owns the dynamic SQL needed to keep group counts and paged
//! group contents on the same filter predicate. Callers only choose the group
//! dimension and provide validated filters.

use fubbik_core::error::AppResult;
use sqlx::{PgPool, Postgres, QueryBuilder};

use super::chunk::Chunk;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupBy {
    Type,
    Status,
    Origin,
    Freshness,
    TagType,
}

#[derive(Debug, Clone)]
pub struct GroupConstraint {
    pub group_by: GroupBy,
    pub group_name: String,
    pub tag_type_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct GroupFilters {
    pub user_id: String,
    pub chunk_type: Option<String>,
    pub origin: Option<String>,
    pub review_status: Option<String>,
    pub space_id: Option<String>,
    pub workspace_id: Option<String>,
    pub global_only: bool,
    pub tags: Vec<String>,
    pub all_tags: bool,
}

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GroupCount {
    pub group_name: String,
    pub count: i64,
}

fn group_expression(group_by: GroupBy) -> &'static str {
    match group_by {
        GroupBy::Type => "c.type",
        GroupBy::Status => "COALESCE(c.review_status, 'draft')",
        GroupBy::Origin => "COALESCE(c.origin, 'human')",
        GroupBy::Freshness => {
            "CASE WHEN c.updated_at >= NOW() - INTERVAL '7 days' THEN 'This week' \
             WHEN c.updated_at >= NOW() - INTERVAL '30 days' THEN 'This month' \
             WHEN c.updated_at >= NOW() - INTERVAL '90 days' THEN 'Last 3 months' \
             ELSE 'Older' END"
        }
        GroupBy::TagType => "t.name",
    }
}

fn push_base_filters<'a>(query: &mut QueryBuilder<'a, Postgres>, filters: &'a GroupFilters) {
    query.push("c.user_id = ").push_bind(&filters.user_id);
    query.push(" AND c.archived_at IS NULL");
    if let Some(chunk_type) = &filters.chunk_type {
        query.push(" AND c.type = ").push_bind(chunk_type);
    }
    if let Some(origin) = &filters.origin {
        query.push(" AND c.origin = ").push_bind(origin);
    }
    if let Some(review_status) = &filters.review_status {
        query
            .push(" AND c.review_status = ")
            .push_bind(review_status);
    }
    if !filters.tags.is_empty() {
        query.push(
            " AND c.id IN (SELECT filtered_ct.chunk_id FROM chunk_tag filtered_ct \
             JOIN tag filtered_t ON filtered_t.id = filtered_ct.tag_id \
             WHERE filtered_t.name = ANY(",
        );
        query.push_bind(&filters.tags).push(")");
        if filters.all_tags {
            query
                .push(" GROUP BY filtered_ct.chunk_id HAVING COUNT(DISTINCT filtered_t.name) = ")
                .push_bind(filters.tags.len() as i64);
        }
        query.push(")");
    }
    if let Some(workspace_id) = &filters.workspace_id {
        query.push(
            " AND (c.id IN (SELECT cs.chunk_id FROM chunk_space cs \
             WHERE cs.space_id IN (SELECT ws.space_id FROM workspace_space ws WHERE ws.workspace_id = ",
        );
        query
            .push_bind(workspace_id)
            .push(")) OR c.id NOT IN (SELECT any_cs.chunk_id FROM chunk_space any_cs))");
    } else if let Some(space_id) = &filters.space_id {
        query.push(" AND (c.id IN (SELECT cs.chunk_id FROM chunk_space cs WHERE cs.space_id = ");
        query
            .push_bind(space_id)
            .push(") OR c.id NOT IN (SELECT any_cs.chunk_id FROM chunk_space any_cs))");
    }
    if filters.global_only {
        query.push(" AND c.id NOT IN (SELECT global_cs.chunk_id FROM chunk_space global_cs)");
    }
}

fn push_constraint<'a>(query: &mut QueryBuilder<'a, Postgres>, constraint: &'a GroupConstraint) {
    match constraint.group_by {
        GroupBy::Type => {
            query
                .push(" AND c.type = ")
                .push_bind(&constraint.group_name);
        }
        GroupBy::Status => {
            query
                .push(" AND COALESCE(c.review_status, 'draft') = ")
                .push_bind(&constraint.group_name);
        }
        GroupBy::Origin => {
            query
                .push(" AND COALESCE(c.origin, 'human') = ")
                .push_bind(&constraint.group_name);
        }
        GroupBy::Freshness => {
            match constraint.group_name.as_str() {
                "This week" => query.push(" AND c.updated_at >= NOW() - INTERVAL '7 days'"),
                "This month" => query.push(
                    " AND c.updated_at >= NOW() - INTERVAL '30 days' \
                     AND c.updated_at < NOW() - INTERVAL '7 days'",
                ),
                "Last 3 months" => query.push(
                    " AND c.updated_at >= NOW() - INTERVAL '90 days' \
                     AND c.updated_at < NOW() - INTERVAL '30 days'",
                ),
                _ => query.push(" AND c.updated_at < NOW() - INTERVAL '90 days'"),
            };
        }
        GroupBy::TagType => {
            query.push(
                " AND c.id IN (SELECT grouped_ct.chunk_id FROM chunk_tag grouped_ct \
                 JOIN tag grouped_t ON grouped_t.id = grouped_ct.tag_id \
                 WHERE grouped_t.name = ",
            );
            query.push_bind(&constraint.group_name);
            if let Some(tag_type_id) = &constraint.tag_type_id {
                query
                    .push(" AND grouped_t.tag_type_id = ")
                    .push_bind(tag_type_id);
            }
            query.push(")");
        }
    }
}

pub async fn grouped_counts(
    pool: &PgPool,
    filters: &GroupFilters,
    group_by: GroupBy,
    tag_type_id: Option<&str>,
    parent: Option<&GroupConstraint>,
) -> AppResult<Vec<GroupCount>> {
    let expression = group_expression(group_by);
    let mut query = QueryBuilder::new("SELECT ");
    query
        .push(expression)
        .push(" AS group_name, ")
        .push(if group_by == GroupBy::TagType {
            "COUNT(DISTINCT c.id)::bigint AS count FROM chunk c \
             JOIN chunk_tag ct ON ct.chunk_id = c.id JOIN tag t ON t.id = ct.tag_id WHERE "
        } else {
            "COUNT(*)::bigint AS count FROM chunk c WHERE "
        });
    push_base_filters(&mut query, filters);
    if group_by == GroupBy::TagType
        && let Some(tag_type_id) = tag_type_id
    {
        query.push(" AND t.tag_type_id = ").push_bind(tag_type_id);
    }
    if let Some(parent) = parent {
        push_constraint(&mut query, parent);
    }
    query.push(" GROUP BY ").push(expression);

    Ok(query.build_query_as::<GroupCount>().fetch_all(pool).await?)
}

fn chunk_select() -> &'static str {
    "SELECT c.id, c.title, c.content, c.type AS chunk_type, c.user_id, c.summary, \
     c.aliases, c.not_about, c.scope, c.rationale, c.alternatives, c.consequences, \
     c.embedding::text AS embedding, c.embedding_updated_at, c.origin, c.review_status, \
     c.reviewed_by, c.reviewed_at, c.created_at, c.updated_at, c.archived_at, \
     c.document_id, c.document_order, c.is_entry_point FROM chunk c WHERE "
}

pub async fn chunks_in_group(
    pool: &PgPool,
    filters: &GroupFilters,
    constraint: &GroupConstraint,
    limit: i64,
    offset: i64,
) -> AppResult<(Vec<Chunk>, i64)> {
    let mut rows_query = QueryBuilder::new(chunk_select());
    push_base_filters(&mut rows_query, filters);
    push_constraint(&mut rows_query, constraint);
    rows_query
        .push(" ORDER BY c.updated_at DESC LIMIT ")
        .push_bind(limit)
        .push(" OFFSET ")
        .push_bind(offset);
    let chunks = rows_query.build_query_as::<Chunk>().fetch_all(pool).await?;

    let mut count_query = QueryBuilder::new("SELECT COUNT(*)::bigint FROM chunk c WHERE ");
    push_base_filters(&mut count_query, filters);
    push_constraint(&mut count_query, constraint);
    let total = count_query
        .build_query_scalar::<i64>()
        .fetch_one(pool)
        .await?;
    Ok((chunks, total))
}
