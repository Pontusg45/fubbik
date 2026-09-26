//! Computed matrix-view contract shared by the API and CLI.

/// Computed status of one cell. Derived on read rather than stored, so it
/// always reflects current requirement statuses and test results.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, utoipa::ToSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum CellStatus {
    Specified,
    Unspecified,
    Violated,
    Verified,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ViewCell {
    pub id: String,
    pub status: CellStatus,
    pub requirement_count: i64,
    pub code_count: i64,
    pub passing_test_count: i64,
    pub failing_test_count: i64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ViewSummary {
    pub specified: i64,
    pub unspecified: i64,
    pub violated: i64,
    pub verified: i64,
    pub total: i64,
}

#[cfg(test)]
mod tests {
    use super::{CellStatus, ViewCell, ViewSummary};

    #[test]
    fn matrix_view_wire_shapes_round_trip() {
        // Given
        let payload = serde_json::json!({
            "cell": {
                "id": "cell-1", "status": "verified", "requirementCount": 0,
                "codeCount": 1, "passingTestCount": 1, "failingTestCount": 0
            },
            "summary": {
                "specified": 0, "unspecified": 0, "violated": 0,
                "verified": 1, "total": 1
            }
        });

        // When
        let cell: ViewCell = serde_json::from_value(payload["cell"].clone()).unwrap();
        let summary: ViewSummary = serde_json::from_value(payload["summary"].clone()).unwrap();

        // Then
        assert_eq!(cell.status, CellStatus::Verified);
        assert_eq!(summary.total, 1);
        assert_eq!(serde_json::to_value(cell).unwrap(), payload["cell"]);
        assert_eq!(serde_json::to_value(summary).unwrap(), payload["summary"]);
    }
}
