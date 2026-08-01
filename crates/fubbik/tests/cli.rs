use std::process::Command;

#[test]
fn help_lists_serve_and_mcp() {
    let out = Command::new(env!("CARGO_BIN_EXE_fubbik"))
        .arg("--help")
        .output()
        .expect("binary runs");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("serve"),
        "missing serve subcommand:\n{stdout}"
    );
    assert!(stdout.contains("mcp"), "missing mcp subcommand:\n{stdout}");
}
