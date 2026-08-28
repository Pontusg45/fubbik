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
    assert!(
        stdout.contains("review"),
        "missing review subcommand:\n{stdout}"
    );
    assert!(
        stdout.contains("plan"),
        "missing plan subcommand:\n{stdout}"
    );
    assert!(
        stdout.contains("task"),
        "missing task subcommand:\n{stdout}"
    );
    assert!(
        stdout.contains("plugin"),
        "missing plugin subcommand:\n{stdout}"
    );
}

#[test]
fn nested_command_help_exposes_the_first_management_slice() {
    for (group, expected) in [
        ("review", ["list", "show", "approve", "reject"].as_slice()),
        ("plan", ["list", "show", "create", "status"].as_slice()),
        ("task", ["add", "list", "claim", "done"].as_slice()),
        ("plugin", ["list", "doctor"].as_slice()),
    ] {
        let out = Command::new(env!("CARGO_BIN_EXE_fubbik"))
            .args([group, "--help"])
            .output()
            .expect("binary runs");
        assert!(out.status.success(), "{group} --help failed");
        let stdout = String::from_utf8_lossy(&out.stdout);
        for command in expected {
            assert!(
                stdout.contains(command),
                "{group} help missing {command}:\n{stdout}"
            );
        }
    }
}

#[cfg(unix)]
#[test]
fn unknown_subcommand_dispatches_to_a_fubbik_plugin() {
    use std::os::unix::fs::PermissionsExt;

    let dir = std::env::temp_dir().join(format!("fubbik-plugin-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create plugin test directory");
    let plugin = dir.join("fubbik-echo-test");
    std::fs::write(
        &plugin,
        "#!/bin/sh\nprintf '%s|%s|%s|%s|%s\\n' \"$1\" \"$2\" \"$FUBBIK_PLUGIN_PROTOCOL\" \"$FUBBIK_OUTPUT\" \"$FUBBIK_URL\"\n",
    )
    .expect("write plugin");
    let mut permissions = std::fs::metadata(&plugin).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&plugin, permissions).unwrap();

    let out = Command::new(env!("CARGO_BIN_EXE_fubbik"))
        .args(["--json", "echo-test", "hello"])
        .env("FUBBIK_PLUGIN_PATH", &dir)
        .output()
        .expect("binary runs");

    assert!(
        out.status.success(),
        "plugin failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "echo-test|hello|1|json|http://localhost:3100\n"
    );

    std::fs::remove_file(plugin).ok();
    std::fs::remove_dir(dir).ok();
}
