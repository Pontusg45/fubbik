//! `GET /api/timeline` — chunk creations and edits over a window.

use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use fubbik_db::repo::insights::{self, TimelineEvent};

use crate::AppState;
use crate::auth::CurrentUser;
use crate::error::ApiResult;
use crate::extract::Query;

#[derive(serde::Deserialize, utoipa::IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct TimelineQuery {
    /// `30d`, `2w`, `6m`, `1y`. Anything unparsable falls back to 30 days
    /// rather than erroring — see [`parse_range`].
    pub range: Option<String>,
    pub space_id: Option<String>,
    pub tag: Option<String>,
}

#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct TimelineTotals {
    pub created: usize,
    pub updated: usize,
}

#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct TimelineRange {
    pub from: String,
    pub to: String,
    pub days: i64,
}

#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct TimelineResponse {
    pub events: Vec<TimelineEvent>,
    pub totals: TimelineTotals,
    pub range: TimelineRange,
}

/// `<n><d|w|m|y>` to days. A malformed range yields 30 rather than an error,
/// matching Node's `if (!match) return 30` — this is a dashboard filter, and
/// failing the whole request over a typo'd query param is worse than showing
/// the default window.
///
/// Months are 30 days and years 365, as in Node. Approximate on purpose:
/// the window is a display convenience, not an accounting boundary.
pub fn parse_range(range: &str) -> i64 {
    let (digits, unit) = range.split_at(range.len().saturating_sub(1));
    let Ok(n) = digits.parse::<i64>() else {
        return 30;
    };
    if n <= 0 {
        return 30;
    }
    match unit {
        "d" => n,
        "w" => n * 7,
        "m" => n * 30,
        "y" => n * 365,
        _ => 30,
    }
}

#[utoipa::path(get, path = "/api/timeline", params(TimelineQuery),
    responses((status = 200, body = TimelineResponse)))]
pub async fn get_timeline(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Query(query): Query<TimelineQuery>,
) -> ApiResult<Json<TimelineResponse>> {
    let days = parse_range(query.range.as_deref().unwrap_or("30d"));
    let now = chrono::Utc::now().naive_utc();
    let from = now - chrono::Duration::days(days);

    let events = insights::timeline(
        &state.pool,
        &user.id,
        from,
        query.space_id.as_deref(),
        query.tag.as_deref(),
    )
    .await?;

    let created = events.iter().filter(|e| e.kind == "created").count();
    Ok(Json(TimelineResponse {
        totals: TimelineTotals {
            created,
            // Node counts anything that is not `created` as updated rather
            // than matching on `"updated"`; the union produces only those two
            // kinds, so the two are equivalent — this is the clearer form.
            updated: events.len() - created,
        },
        range: TimelineRange {
            from: from.and_utc().to_rfc3339(),
            to: now.and_utc().to_rfc3339(),
            days,
        },
        events,
    }))
}

pub fn router() -> Router<AppState> {
    Router::new().route("/api/timeline", get(get_timeline))
}

#[cfg(test)]
mod tests {
    use super::parse_range;

    /// Every unit, plus the fallbacks. Enumerated because each is a separate
    /// match arm and a wrong multiplier is invisible from testing another.
    #[test]
    fn range_parsing() {
        // Given the inline inputs and test fixtures.
        // When the operation is evaluated by the assertion.
        // Then
        assert_eq!(parse_range("7d"), 7);
        assert_eq!(parse_range("2w"), 14);
        assert_eq!(parse_range("6m"), 180);
        assert_eq!(parse_range("1y"), 365);
        for bad in ["", "d", "abc", "30x", "-5d", "0d", "30"] {
            assert_eq!(parse_range(bad), 30, "`{bad}` must fall back to 30 days");
        }
    }
}
