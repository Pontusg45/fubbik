use std::{path::Path, process::Command};

#[test]
fn source_previews_run_without_external_documentation_tools() {
    // Given the source fixtures and a PATH with no Node, JSDoc, TypeDoc, or JDK
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../fubbik-cli/tests/fixtures/source-docs");

    for (language, key, signature) in [
        (
            "javascript",
            "example.js::find",
            "find(name: string): string",
        ),
        (
            "typescript",
            "example.ts::find(string)",
            "find(name: string): string",
        ),
        ("java", "sample.Lookup", "sample.Lookup"),
    ] {
        // When the shipped CLI previews a native source extraction
        let output = Command::new(env!("CARGO_BIN_EXE_fubbik"))
            .args(["docs", "extract"])
            .arg(&fixture)
            .args(["--language", language, "--project", "fixture", "--preview"])
            .env("PATH", "/nonexistent")
            .output()
            .unwrap();

        // Then it emits a complete, validated source manifest without a subprocess
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let manifest: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(manifest["complete"], true);
        assert_eq!(manifest["symbols"][0]["key"], key);
        assert_eq!(manifest["symbols"][0]["signature"], signature);
    }
}
