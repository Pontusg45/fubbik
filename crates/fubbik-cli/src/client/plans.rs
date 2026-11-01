use super::*;

impl Client {
    pub async fn list_plans(
        &self,
        status: Option<&str>,
        space_id: Option<&str>,
    ) -> Result<Vec<Plan>> {
        let mut query = Vec::new();
        if let Some(status) = status {
            query.push(("status", status.to_string()));
        }
        if let Some(space_id) = space_id {
            query.push(("spaceId", space_id.to_string()));
        }
        self.get_json("/api/plans", &query).await
    }

    pub async fn get_plan(&self, id: &str) -> Result<PlanDetail> {
        self.get_json(&format!("/api/plans/{id}"), &[]).await
    }

    pub async fn create_plan(
        &self,
        title: &str,
        description: Option<&str>,
        space_id: Option<&str>,
        tasks: &[&str],
    ) -> Result<Plan> {
        let tasks: Vec<_> = tasks
            .iter()
            .map(|title| serde_json::json!({ "title": title }))
            .collect();
        self.send_json(
            reqwest::Method::POST,
            "/api/plans",
            serde_json::json!({
                "title": title,
                "description": description,
                "spaceId": space_id,
                "tasks": tasks,
            }),
        )
        .await
    }

    pub async fn update_plan_status(&self, id: &str, status: &str) -> Result<Plan> {
        self.send_json(
            reqwest::Method::PATCH,
            &format!("/api/plans/{id}"),
            serde_json::json!({ "status": status }),
        )
        .await
    }

    pub async fn create_plan_task(
        &self,
        plan_id: &str,
        title: &str,
        description: Option<&str>,
    ) -> Result<PlanTask> {
        self.send_json(
            reqwest::Method::POST,
            &format!("/api/plans/{plan_id}/tasks"),
            serde_json::json!({ "title": title, "description": description }),
        )
        .await
    }

    pub async fn link_plan_requirement(
        &self,
        plan_id: &str,
        requirement_id: &str,
    ) -> Result<serde_json::Value> {
        self.send_json(
            reqwest::Method::POST,
            &format!("/api/plans/{plan_id}/requirements"),
            serde_json::json!({ "requirementId": requirement_id }),
        )
        .await
    }

    pub async fn create_quick_task(
        &self,
        title: &str,
        description: Option<&str>,
        space_id: Option<&str>,
        priority: &str,
    ) -> Result<Plan> {
        self.send_json(
            reqwest::Method::POST,
            "/api/tasks",
            serde_json::json!({
                "title": title,
                "description": description,
                "spaceId": space_id,
                "priority": priority,
            }),
        )
        .await
    }

    pub async fn list_quick_tasks(&self, status: &str) -> Result<Vec<Plan>> {
        self.get_json("/api/tasks", &[("status", status.to_owned())])
            .await
    }

    pub async fn update_task_status(
        &self,
        plan_id: &str,
        task_id: &str,
        status: &str,
    ) -> Result<PlanTask> {
        self.send_json(
            reqwest::Method::PATCH,
            &format!("/api/plans/{plan_id}/tasks/{task_id}"),
            serde_json::json!({ "status": status }),
        )
        .await
    }

    pub async fn claim_quick_task(&self, plan_id: &str) -> Result<PlanDetail> {
        self.send_json(
            reqwest::Method::POST,
            &format!("/api/tasks/{plan_id}/claim"),
            serde_json::Value::Null,
        )
        .await
    }

    pub async fn complete_quick_task(&self, plan_id: &str, note: Option<&str>) -> Result<Plan> {
        self.send_json(
            reqwest::Method::POST,
            &format!("/api/tasks/{plan_id}/complete"),
            serde_json::json!({ "note": note }),
        )
        .await
    }
}
