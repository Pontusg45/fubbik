use super::*;

impl Client {
    pub async fn list_requirements(
        &self,
        space_id: Option<&str>,
        status: Option<&str>,
        priority: Option<&str>,
    ) -> Result<Vec<Requirement>> {
        let mut query = Vec::new();
        if let Some(value) = space_id {
            query.push(("spaceId", value.to_owned()));
        }
        if let Some(value) = status {
            query.push(("status", value.to_owned()));
        }
        if let Some(value) = priority {
            query.push(("priority", value.to_owned()));
        }
        let response: RequirementListResponse = self.get_json("/api/requirements", &query).await?;
        Ok(response.requirements)
    }

    pub async fn create_requirement(
        &self,
        title: &str,
        description: Option<&str>,
        steps: &[serde_json::Value],
        space_id: Option<&str>,
        priority: Option<&str>,
    ) -> Result<Requirement> {
        let response: RequirementMutationResponse = self
            .send_json(
                reqwest::Method::POST,
                "/api/requirements",
                serde_json::json!({
                    "title": title,
                    "description": description,
                    "steps": steps,
                    "spaceId": space_id,
                    "priority": priority,
                }),
            )
            .await?;
        Ok(response.requirement)
    }

    pub async fn update_requirement_status(&self, id: &str, status: &str) -> Result<Requirement> {
        self.send_json(
            reqwest::Method::PATCH,
            &format!("/api/requirements/{id}/status"),
            serde_json::json!({ "status": status }),
        )
        .await
    }

    pub async fn export_requirements(
        &self,
        format: &str,
        space_id: Option<&str>,
    ) -> Result<String> {
        let mut query = vec![("format", format.to_owned())];
        if let Some(value) = space_id {
            query.push(("spaceId", value.to_owned()));
        }
        self.get_text("/api/requirements/export", &query).await
    }
}
