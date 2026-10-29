use std::fs;
use std::process::Command;

#[test]
fn init_refuses_to_overwrite() {
    // Given
    let dir = std::env::temp_dir().join(format!("review-harness-init-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let bin = env!("CARGO_BIN_EXE_review-harness");
    // When
    let first = Command::new(bin)
        .args(["init", "--root"])
        .arg(&dir)
        .output()
        .unwrap();
    let second = Command::new(bin)
        .args(["init", "--root"])
        .arg(&dir)
        .output()
        .unwrap();
    // Then
    assert!(first.status.success());
    assert!(dir.join("review-rules.toml").is_file());
    assert!(!second.status.success());
    let stderr = String::from_utf8_lossy(&second.stderr);
    assert!(
        stderr.contains("review-rules.toml already exists"),
        "{stderr}"
    );
    fs::remove_dir_all(dir).unwrap();
}
