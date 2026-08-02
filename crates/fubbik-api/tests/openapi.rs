use utoipa::OpenApi;

#[test]
fn document_contains_every_chunk_path() {
    let doc = fubbik_api::openapi::ApiDoc::openapi();
    let json = serde_json::to_value(&doc).unwrap();
    let paths = json["paths"].as_object().unwrap();

    for expected in [
        "/api/chunks",
        "/api/chunks/{id}",
        "/api/chunks/{id}/history",
        "/api/chunks/{id}/applies-to",
        "/api/chunks/{id}/file-refs",
    ] {
        assert!(paths.contains_key(expected), "missing path {expected}");
    }
}

#[test]
fn committed_openapi_json_is_current() {
    let doc = fubbik_api::openapi::ApiDoc::openapi();
    let generated = serde_json::to_string_pretty(&doc).unwrap();
    let committed =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../openapi.json"))
            .expect("openapi.json exists at repo root");

    assert_eq!(
        generated.trim(),
        committed.trim(),
        "openapi.json is stale — run `cargo run -- openapi > openapi.json`"
    );
}
