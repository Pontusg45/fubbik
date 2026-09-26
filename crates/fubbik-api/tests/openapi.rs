use utoipa::OpenApi;

#[test]
fn operation_ids_are_unique() {
    // Given
    let doc = fubbik_api::openapi::ApiDoc::openapi();
    let json = serde_json::to_value(&doc).unwrap();

    // When
    let mut seen = std::collections::HashMap::new();
    let mut duplicates = Vec::new();
    for (path, methods) in json["paths"].as_object().unwrap() {
        for (method, operation) in methods.as_object().unwrap() {
            if let Some(id) = operation["operationId"].as_str()
                && let Some(previous) = seen.insert(id, format!("{method} {path}"))
            {
                duplicates.push(format!("{id}: {previous}, {method} {path}"));
            }
        }
    }

    // Then
    assert!(
        duplicates.is_empty(),
        "duplicate operation IDs: {duplicates:?}"
    );
}

#[test]
fn document_contains_every_chunk_path() {
    // Given
    let doc = fubbik_api::openapi::ApiDoc::openapi();
    let json = serde_json::to_value(&doc).unwrap();
    // When
    let paths = json["paths"].as_object().unwrap();

    for expected in [
        "/api/chunks",
        "/api/chunks/{id}",
        "/api/chunks/{id}/history",
        "/api/chunks/{id}/applies-to",
        "/api/chunks/{id}/file-refs",
    ] {
        // Then
        assert!(paths.contains_key(expected), "missing path {expected}");
    }
}

#[test]
fn committed_openapi_json_is_current() {
    // Given
    let doc = fubbik_api::openapi::ApiDoc::openapi();
    let generated = serde_json::to_string_pretty(&doc).unwrap();
    // When
    let committed =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../openapi.json"))
            .expect("openapi.json exists at repo root");

    // Then
    assert_eq!(
        generated.trim(),
        committed.trim(),
        "openapi.json is stale — run `cargo run -- openapi > openapi.json`"
    );
}
