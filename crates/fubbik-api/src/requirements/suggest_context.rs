//! Context aggregation for AI-assisted requirement suggestions.

use fubbik_core::error::AppResult;
use fubbik_db::repo::{chunk, coverage, knowledge_health, requirement, use_case};
use sqlx::PgPool;

#[derive(serde::Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct SuggestContextQuery {
    pub focus: Option<String>,
    pub space_id: Option<String>,
}

#[derive(serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SuggestRequirement {
    pub id: String,
    pub title: String,
    pub status: String,
    pub priority: Option<String>,
}

#[derive(serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SuggestUseCase {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub requirements: Vec<SuggestRequirement>,
}

#[derive(serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CoverageGap {
    pub id: String,
    pub title: String,
}

#[derive(serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct HealthIssueCounts {
    pub orphan: i64,
    pub stale: i64,
    pub thin: i64,
}

#[derive(serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RelevantChunk {
    pub id: String,
    pub title: String,
    pub content: String,
    #[serde(rename = "type")]
    pub chunk_type: String,
}

#[derive(serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SuggestContextResponse {
    pub use_cases: Vec<SuggestUseCase>,
    pub ungrouped_requirements: Vec<SuggestRequirement>,
    pub coverage_gaps: Vec<CoverageGap>,
    pub health_issue_counts: HealthIssueCounts,
    pub relevant_chunks: Vec<RelevantChunk>,
}

pub async fn get_suggest_context(
    pool: &PgPool,
    user_id: &str,
    query: SuggestContextQuery,
) -> AppResult<SuggestContextResponse> {
    let focus = query.focus.as_deref().map(str::to_lowercase);
    let space_id = query.space_id.as_deref();

    let mut use_cases = Vec::new();
    for item in use_case::list(pool, user_id, space_id).await? {
        let requirements = use_case::list_requirements(pool, user_id, &item.id)
            .await?
            .into_iter()
            .filter(|requirement| matches_focus(&requirement.title, focus.as_deref()))
            .map(|requirement| SuggestRequirement {
                id: requirement.id,
                title: requirement.title,
                status: requirement.status,
                priority: requirement.priority,
            })
            .collect::<Vec<_>>();
        if focus.is_some() && requirements.is_empty() {
            continue;
        }
        use_cases.push(SuggestUseCase {
            id: item.id,
            name: item.name,
            description: item.description,
            requirements,
        });
    }

    let requirement_params = |offset| requirement::ListParams {
        space_id,
        use_case_id: None,
        status: None,
        priority: None,
        origin: None,
        review_status: None,
        search: None,
        limit: 100,
        offset,
    };
    let mut requirements = requirement::list(pool, user_id, &requirement_params(0)).await?;
    requirements.extend(requirement::list(pool, user_id, &requirement_params(100)).await?);
    let ungrouped_requirements = requirements
        .into_iter()
        .filter(|requirement| requirement.use_case_id.is_none())
        .filter(|requirement| matches_focus(&requirement.title, focus.as_deref()))
        .map(|requirement| SuggestRequirement {
            id: requirement.id,
            title: requirement.title,
            status: requirement.status,
            priority: requirement.priority,
        })
        .collect();

    let gap_limit = if focus.is_some() { 20 } else { 10 };
    let coverage_gaps = coverage::get_chunk_coverage(pool, user_id, space_id)
        .await?
        .into_iter()
        .filter(|row| row.requirement_count == 0)
        .filter(|row| matches_focus(&row.title, focus.as_deref()))
        .take(gap_limit)
        .map(|row| CoverageGap {
            id: row.id,
            title: row.title,
        })
        .collect();

    let (orphans, stale, thin) = tokio::try_join!(
        knowledge_health::orphan_chunks(pool, user_id, space_id),
        knowledge_health::stale_chunks(pool, user_id, space_id),
        knowledge_health::thin_chunks(pool, user_id, space_id),
    )?;

    let relevant_chunks = if let Some(focus) = query.focus {
        let params = chunk::ListParams {
            search: Some(focus),
            space_id: query.space_id,
            limit: 20,
            ..Default::default()
        };
        chunk::list(pool, user_id, &params)
            .await?
            .into_iter()
            .map(|item| RelevantChunk {
                id: item.id,
                title: item.title,
                content: item.content.chars().take(300).collect(),
                chunk_type: item.chunk_type,
            })
            .collect()
    } else {
        Vec::new()
    };

    Ok(SuggestContextResponse {
        use_cases,
        ungrouped_requirements,
        coverage_gaps,
        health_issue_counts: HealthIssueCounts {
            orphan: orphans.count,
            stale: stale.count,
            thin: thin.count,
        },
        relevant_chunks,
    })
}

fn matches_focus(title: &str, focus: Option<&str>) -> bool {
    focus.is_none_or(|focus| title.to_lowercase().contains(focus))
}
