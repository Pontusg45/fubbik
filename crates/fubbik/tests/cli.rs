use std::process::Command;

#[test]
fn help_lists_serve_and_mcp() {
    // Given the inline inputs and test fixtures.
    // When
    let out = Command::new(env!("CARGO_BIN_EXE_fubbik"))
        .arg("--help")
        .output()
        .expect("binary runs");
    let stdout = String::from_utf8_lossy(&out.stdout);
    // Then
    assert!(
        stdout.contains("serve"),
        "missing serve subcommand:\n{stdout}"
    );
    assert!(stdout.contains("mcp"), "missing mcp subcommand:\n{stdout}");
    assert!(
        stdout.contains("mcp-tools"),
        "missing mcp-tools subcommand:\n{stdout}"
    );
    assert!(
        stdout.contains("review"),
        "missing review subcommand:\n{stdout}"
    );
    assert!(
        stdout.contains("quick"),
        "missing quick subcommand:\n{stdout}"
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
    for command in [
        "space", "tag", "link", "unlink", "req", "stats", "enrich", "stale", "status", "docs",
        "chunk", "open", "generate", "updates", "why", "suggest", "gaps", "recap", "prompt",
        "kb-diff",
    ] {
        assert!(
            stdout.contains(command),
            "missing {command} subcommand:\n{stdout}"
        );
    }
}

#[test]
fn mcp_tools_json_uses_the_live_rust_catalog() {
    // Given the Rust CLI in machine-readable mode
    // When the MCP tool catalog is requested
    let out = Command::new(env!("CARGO_BIN_EXE_fubbik"))
        .args(["--json", "mcp-tools"])
        .output()
        .expect("binary runs");

    // Then the complete live tool catalog is returned
    assert!(out.status.success());
    let tools: Vec<serde_json::Value> = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(tools.len(), 53);
    assert!(tools.iter().any(|tool| tool["name"] == "search_chunks"));
    assert!(tools.iter().any(|tool| tool["name"] == "sync_claude_md"));
}

#[test]
fn nested_command_help_exposes_the_first_management_slice() {
    for (group, expected) in [
        ("review", ["list", "show", "approve", "reject"].as_slice()),
        (
            "plan",
            [
                "list",
                "show",
                "create",
                "status",
                "add-task",
                "task-done",
                "link-requirement",
            ]
            .as_slice(),
        ),
        ("task", ["add", "list", "claim", "done"].as_slice()),
        ("plugin", ["list", "doctor"].as_slice()),
        ("prompt", ["list", "get", "add"].as_slice()),
        ("space", ["list", "add", "remove", "current"].as_slice()),
        ("tag", ["list", "add", "rename", "remove"].as_slice()),
        (
            "req",
            ["list", "add", "status", "export", "verify", "import"].as_slice(),
        ),
        ("stale", ["list", "dismiss"].as_slice()),
        (
            "docs",
            ["list", "show", "import", "import-dir", "sync", "render"].as_slice(),
        ),
        (
            "generate",
            ["claude.md", "agents.md", "cursorrules"].as_slice(),
        ),
        (
            "chunk",
            [
                "add", "get", "cat", "update", "remove", "list", "search", "link", "unlink",
            ]
            .as_slice(),
        ),
    ] {
        // Given the inline inputs and test fixtures.
        // When
        let out = Command::new(env!("CARGO_BIN_EXE_fubbik"))
            .args([group, "--help"])
            .output()
            .expect("binary runs");
        // Then
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

#[test]
fn context_help_exposes_semantic_about_lookup() {
    // Given the Rust CLI binary
    // When context help is requested
    let out = Command::new(env!("CARGO_BIN_EXE_fubbik"))
        .args(["context", "--help"])
        .output()
        .expect("binary runs");

    // Then the migrated semantic lookup is available
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("about"));
    assert!(stdout.contains("for-plan"));
    assert!(stdout.contains("for-diff"));
    assert!(stdout.contains("snapshot"));
    assert!(stdout.contains("dir"));
}

#[test]
fn context_snapshot_help_exposes_the_full_lifecycle() {
    // Given the Rust CLI binary
    // When snapshot help is requested
    let out = Command::new(env!("CARGO_BIN_EXE_fubbik"))
        .args(["context", "snapshot", "--help"])
        .output()
        .expect("binary runs");

    // Then all legacy snapshot operations are available
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    for operation in ["create", "get", "list", "delete"] {
        assert!(stdout.contains(operation), "missing {operation}:\n{stdout}");
    }
}

#[test]
fn sync_claude_md_alias_exposes_watch_and_global_options() {
    // Given the legacy CLAUDE.md sync command name
    // When its help is requested through the Rust binary
    let out = Command::new(env!("CARGO_BIN_EXE_fubbik"))
        .args(["sync-claude-md", "--help"])
        .output()
        .expect("binary runs");

    // Then the compatibility alias and its retained modes are available
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    for option in ["--watch", "--interval", "--global", "--tag", "--output"] {
        assert!(stdout.contains(option), "missing {option}:\n{stdout}");
    }
}

#[test]
fn open_json_reports_the_target_without_launching_a_browser() {
    // Given no server configuration requirement and machine-readable output
    // When a named web page is opened
    let out = Command::new(env!("CARGO_BIN_EXE_fubbik"))
        .args(["--json", "open", "graph"])
        .env("FUBBIK_WEB_URL", "https://fubbik.test/")
        .output()
        .expect("binary runs");

    // Then the resolved URL is returned without needing the API server
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(value["url"], "https://fubbik.test/graph");
}

#[test]
fn mcp_serves_initialize_and_tool_catalog_over_stdio() {
    // Given a spawned Rust MCP process and two newline-delimited requests
    use std::io::Write;
    use std::process::Stdio;

    let mut child = Command::new(env!("CARGO_BIN_EXE_fubbik"))
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("MCP process starts");
    let stdin = child.stdin.as_mut().unwrap();
    stdin
        .write_all(
            br#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}
{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}
"#,
        )
        .unwrap();
    drop(child.stdin.take());

    // When the process reaches EOF
    let output = child.wait_with_output().expect("MCP process exits");

    // Then it returns valid protocol responses and the Rust core tool catalog
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let responses = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(responses.len(), 2);
    assert_eq!(responses[0]["result"]["serverInfo"]["name"], "fubbik");
    assert_eq!(
        responses[1]["result"]["tools"].as_array().unwrap().len(),
        53
    );
}

#[test]
fn space_scoped_commands_keep_the_codebase_option_alias() {
    for args in [
        &["context", "export", "--help"][..],
        &["plan", "list", "--help"],
        &["req", "list", "--help"],
        &["docs", "import", "--help"],
        &["chunk", "add", "--help"],
    ] {
        // Given the inline inputs and test fixtures.
        // When
        let out = Command::new(env!("CARGO_BIN_EXE_fubbik"))
            .args(args)
            .output()
            .expect("binary runs");
        // Then
        assert!(out.status.success(), "{} failed", args.join(" "));
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(
            stdout.contains("--codebase"),
            "{} help missing --codebase alias:\n{stdout}",
            args.join(" ")
        );
    }
}

#[cfg(unix)]
#[test]
fn unknown_subcommand_dispatches_to_a_fubbik_plugin() {
    // Given
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

    // When
    let out = Command::new(env!("CARGO_BIN_EXE_fubbik"))
        .args(["--json", "echo-test", "hello"])
        .env("FUBBIK_PLUGIN_PATH", &dir)
        .output()
        .expect("binary runs");

    // Then
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
