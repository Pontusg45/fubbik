use super::*;

impl Client {
    pub async fn list_documents(&self, space_id: Option<&str>) -> Result<Vec<serde_json::Value>> {
        let query = space_id
            .map(|value| vec![("spaceId", value.to_owned())])
            .unwrap_or_default();
        self.get_json("/api/documents", &query).await
    }

    pub async fn get_document(&self, id: &str) -> Result<serde_json::Value> {
        self.get_json(&format!("/api/documents/{id}"), &[]).await
    }

    pub async fn import_document(
        &self,
        source_path: &str,
        content: &str,
        space_id: Option<&str>,
    ) -> Result<serde_json::Value> {
        self.send_json(
            reqwest::Method::POST,
            "/api/documents/import",
            serde_json::json!({
                "sourcePath": source_path,
                "content": content,
                "spaceId": space_id,
            }),
        )
        .await
    }

    pub async fn import_documents(
        &self,
        files: &[(String, String)],
        space_id: Option<&str>,
    ) -> Result<Vec<serde_json::Value>> {
        let files = files
            .iter()
            .map(|(source_path, content)| {
                serde_json::json!({ "sourcePath": source_path, "content": content })
            })
            .collect::<Vec<_>>();
        self.send_json(
            reqwest::Method::POST,
            "/api/documents/import-dir",
            serde_json::json!({ "files": files, "spaceId": space_id }),
        )
        .await
    }

    pub async fn import_chunk_documents(
        &self,
        files: &[(String, String)],
        space_id: &str,
    ) -> Result<serde_json::Value> {
        let files = files
            .iter()
            .map(|(path, content)| serde_json::json!({"path": path, "content": content}))
            .collect::<Vec<_>>();
        self.send_json(
            reqwest::Method::POST,
            "/api/chunks/import-docs",
            serde_json::json!({"files": files, "spaceId": space_id}),
        )
        .await
    }

    pub async fn import_source_docs(
        &self,
        space_id: &str,
        manifest: &fubbik_core::source_docs::SourceManifest,
    ) -> Result<serde_json::Value> {
        self.send_json(
            reqwest::Method::POST,
            "/api/documents/import-source",
            serde_json::json!({ "spaceId": space_id, "manifest": manifest }),
        )
        .await
    }

    pub async fn sync_document(
        &self,
        id: &str,
        content: &str,
        space_id: Option<&str>,
    ) -> Result<serde_json::Value> {
        self.send_json(
            reqwest::Method::POST,
            &format!("/api/documents/{id}/sync"),
            serde_json::json!({ "content": content, "spaceId": space_id }),
        )
        .await
    }

    pub async fn render_document(&self, id: &str) -> Result<serde_json::Value> {
        self.get_json(&format!("/api/documents/{id}/render"), &[])
            .await
    }
}
