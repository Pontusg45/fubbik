use anyhow::Result;
use fubbik_contracts::matrix::{ViewCell, ViewSummary};

use super::Client;

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Matrix {
    pub id: String,
    pub name: String,
    pub layer: String,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixDimension {
    pub id: String,
    pub name: String,
    pub order: i32,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixRule {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub category: Option<String>,
    pub order: i32,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct MatrixView {
    pub matrix: Matrix,
    pub dimensions: Vec<MatrixDimension>,
    pub rules: Vec<MatrixRule>,
    pub cells: std::collections::HashMap<String, ViewCell>,
    pub summary: ViewSummary,
}

impl Client {
    pub async fn list_matrices(&self, layer: Option<&str>) -> Result<Vec<Matrix>> {
        let query = layer
            .map(|layer| vec![("layer", layer.to_owned())])
            .unwrap_or_default();
        self.get_json("/api/matrices", &query).await
    }

    pub async fn matrix_view(&self, id: &str) -> Result<MatrixView> {
        self.get_json(&format!("/api/matrices/{id}/view"), &[])
            .await
    }

    pub async fn create_matrix(
        &self,
        name: &str,
        layer: &str,
        description: Option<&str>,
        space_id: Option<&str>,
    ) -> Result<Matrix> {
        self.send_json(
            reqwest::Method::POST,
            "/api/matrices",
            serde_json::json!({
                "name": name,
                "layer": layer,
                "description": description,
                "spaceId": space_id,
            }),
        )
        .await
    }

    pub async fn add_matrix_dimension(
        &self,
        matrix_id: &str,
        name: &str,
    ) -> Result<serde_json::Value> {
        self.send_json(
            reqwest::Method::POST,
            &format!("/api/matrices/{matrix_id}/dimensions"),
            serde_json::json!({ "name": name }),
        )
        .await
    }

    pub async fn add_matrix_rule(
        &self,
        matrix_id: &str,
        body: serde_json::Value,
    ) -> Result<serde_json::Value> {
        self.send_json(
            reqwest::Method::POST,
            &format!("/api/matrices/{matrix_id}/rules"),
            body,
        )
        .await
    }

    pub async fn toggle_matrix_cell(
        &self,
        matrix_id: &str,
        rule_id: &str,
        dimension_id: &str,
    ) -> Result<serde_json::Value> {
        self.send_json(
            reqwest::Method::PUT,
            &format!("/api/matrices/{matrix_id}/cells"),
            serde_json::json!({ "ruleId": rule_id, "dimensionId": dimension_id }),
        )
        .await
    }

    pub async fn link_matrix_requirement(
        &self,
        matrix_id: &str,
        cell_id: &str,
        requirement_id: &str,
    ) -> Result<serde_json::Value> {
        self.send_json(
            reqwest::Method::POST,
            &format!("/api/matrices/{matrix_id}/cells/{cell_id}/requirements"),
            serde_json::json!({ "requirementId": requirement_id }),
        )
        .await
    }

    pub async fn link_matrix_code(
        &self,
        matrix_id: &str,
        cell_id: &str,
        kind: &str,
        code_ref: &str,
    ) -> Result<serde_json::Value> {
        self.send_json(
            reqwest::Method::POST,
            &format!("/api/matrices/{matrix_id}/cells/{cell_id}/code"),
            serde_json::json!({ "kind": kind, "ref": code_ref }),
        )
        .await
    }

    pub async fn report_matrix_test(
        &self,
        matrix_id: &str,
        cell_id: &str,
        test_ref: &str,
        status: &str,
        detail: Option<&str>,
    ) -> Result<serde_json::Value> {
        self.send_json(
            reqwest::Method::POST,
            &format!("/api/matrices/{matrix_id}/cells/{cell_id}/test-results"),
            serde_json::json!({ "testRef": test_ref, "status": status, "detail": detail }),
        )
        .await
    }

    pub async fn matrix_rule_history(
        &self,
        matrix_id: &str,
        rule_id: &str,
    ) -> Result<Vec<serde_json::Value>> {
        self.get_json(
            &format!("/api/matrices/{matrix_id}/rules/{rule_id}/history"),
            &[],
        )
        .await
    }

    pub async fn matrix_behaviors_for_file(&self, path: &str) -> Result<Vec<serde_json::Value>> {
        self.get_json(
            "/api/matrices/behaviors-for-file",
            &[("path", path.to_owned())],
        )
        .await
    }
}
