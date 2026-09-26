use super::*;

pub async fn import(client: &Client, space: &Space, discovery: &Discovery) -> ImportReport {
    let space_id = &space.id;
    let directory = &discovery.root;
    let chunks = &discovery.chunks;
    let connections = &discovery.connections;
    let mut result = ImportReport {
        chunks_created: 0,
        connections_created: 0,
        errors: Vec::new(),
    };
    let mut title_to_id = HashMap::new();
    let mut files = BTreeMap::new();
    for chunk in chunks.iter().filter(|chunk| chunk.tier == 1) {
        if let Ok(content) = std::fs::read_to_string(directory.join(&chunk.source)) {
            files.entry(chunk.source.clone()).or_insert(content);
        }
    }
    if !files.is_empty() {
        let files = files.into_iter().collect::<Vec<_>>();
        match client.import_chunk_documents(&files, space_id).await {
            Ok(value) => {
                result.chunks_created += value["created"].as_i64().unwrap_or(0);
                for error in value["errors"].as_array().into_iter().flatten() {
                    result.errors.push(ImportError {
                        item: error["path"].as_str().unwrap_or("import-docs").into(),
                        error: error["error"].as_str().unwrap_or("unknown error").into(),
                    });
                }
            }
            Err(error) => result.errors.push(ImportError {
                item: "import-docs".into(),
                error: error.to_string(),
            }),
        }
    }
    for chunk in chunks.iter().filter(|chunk| chunk.tier != 1) {
        match client
            .create_discovered_chunk(
                &chunk.title,
                &chunk.content,
                &chunk.chunk_type,
                &chunk.tags,
                space_id,
            )
            .await
        {
            Ok(created) => {
                result.chunks_created += 1;
                title_to_id.insert(chunk.title.clone(), created.id.clone());
                if !chunk.applies_to.is_empty() {
                    let _ = client.set_applies_to(&created.id, &chunk.applies_to).await;
                }
            }
            Err(error) => result.errors.push(ImportError {
                item: chunk.title.clone(),
                error: error.to_string(),
            }),
        }
    }
    match client.list_all_chunks(Some(space_id)).await {
        Ok(existing) => {
            for chunk in existing {
                title_to_id.entry(chunk.title).or_insert(chunk.id);
            }
        }
        Err(error) => result.errors.push(ImportError {
            item: "existing chunks".into(),
            error: error.to_string(),
        }),
    }
    for connection in connections {
        let Some(source) = title_to_id.get(&connection.source_title) else {
            continue;
        };
        let Some(target) = title_to_id.get(&connection.target_title) else {
            continue;
        };
        if client
            .create_discovered_connection(source, target, &connection.relation)
            .await
            .is_ok()
        {
            result.connections_created += 1;
        }
    }
    result
}
