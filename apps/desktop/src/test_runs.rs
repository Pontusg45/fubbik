//! Running a repo's tests and reading the results: which runner a repo
//! uses (cargo nextest, cargo test, vitest), the command, and the failing
//! tests with their messages, so the app can show them, rerun just the
//! failures, and hand them to an agent.

use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Runner {
    /// `cargo nextest run`, with structured (libtest-json) results.
    Nextest,
    /// `cargo test`, results read from its text output.
    Cargo,
    /// `vitest run` with its JSON reporter.
    Vitest,
}

impl Runner {
    pub fn label(self) -> &'static str {
        match self {
            Runner::Nextest => "cargo nextest",
            Runner::Cargo => "cargo test",
            Runner::Vitest => "vitest",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    /// The test's name as the runner reports it (`tests::bad`, a vitest
    /// full name).
    pub name: String,
    /// What it printed (the panic, the assertion), trimmed.
    pub message: String,
    /// The file (vitest), to rerun only it.
    pub file: Option<String>,
}

impl Failure {
    /// The first `path:line` in the message (`src/lib.rs:7:16`), for "Open".
    pub fn location(&self) -> Option<(String, u32)> {
        location_in(&self.message)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    pub passed: usize,
    pub failed: usize,
    pub ignored: usize,
    pub failures: Vec<Failure>,
    /// The run did not produce results (it did not compile, the runner is
    /// missing): the last lines of its output.
    pub error: Option<String>,
}

/// Which runners a repo has: Rust when it has a Cargo.toml (nextest when
/// installed), vitest when its package.json mentions it or a vitest config
/// is at the root.
pub fn detect(repo: &Path) -> Vec<Runner> {
    let mut out = Vec::new();
    if repo.join("Cargo.toml").is_file() {
        let nextest = Command::new("cargo")
            .args(["nextest", "--version"])
            .output()
            .is_ok_and(|o| o.status.success());
        out.push(if nextest {
            Runner::Nextest
        } else {
            Runner::Cargo
        });
    }
    let package = std::fs::read_to_string(repo.join("package.json")).unwrap_or_default();
    let vitest_config = [
        "vitest.config.ts",
        "vitest.config.js",
        "vitest.config.mts",
        "vitest.workspace.ts",
    ]
    .iter()
    .any(|f| repo.join(f).is_file());
    if package.contains("\"vitest\"") || vitest_config {
        out.push(Runner::Vitest);
    }
    out
}

/// The package manager's exec for a JS repo (`pnpm exec`, `bunx`, `npx`).
fn js_exec(repo: &Path) -> &'static str {
    if repo.join("pnpm-lock.yaml").is_file() {
        "pnpm exec"
    } else if repo.join("bun.lockb").is_file() || repo.join("bun.lock").is_file() {
        "bunx"
    } else {
        "npx"
    }
}

/// The shell command for a run, and for vitest the file its JSON goes to.
/// `only`: rerun these failures only.
pub fn command(runner: Runner, repo: &Path, only: &[Failure]) -> (String, Option<PathBuf>) {
    let quote = |s: &str| format!("'{}'", s.replace('\'', r"'\''"));
    match runner {
        Runner::Nextest => {
            let filters: Vec<String> = only.iter().map(|f| quote(&f.name)).collect();
            (
                format!(
                    "NEXTEST_EXPERIMENTAL_LIBTEST_JSON=1 cargo nextest run --no-fail-fast --message-format libtest-json {}",
                    filters.join(" ")
                )
                .trim_end()
                .to_string(),
                None,
            )
        }
        Runner::Cargo => {
            // cargo test takes one filter; rerunning many runs them all.
            let filter = match only {
                [one] => format!(" {}", quote(&one.name)),
                _ => String::new(),
            };
            (format!("cargo test --no-fail-fast{filter}"), None)
        }
        Runner::Vitest => {
            let out = std::env::temp_dir().join(format!(
                "fubbik-vitest-{}.json",
                crate::agent_task::uuid_v4()
            ));
            let mut files: Vec<String> = only.iter().filter_map(|f| f.file.clone()).collect();
            files.sort();
            files.dedup();
            let files: Vec<String> = files.iter().map(|f| quote(f)).collect();
            (
                format!(
                    "{} vitest run --reporter=json --outputFile={} {}",
                    js_exec(repo),
                    quote(&out.to_string_lossy()),
                    files.join(" ")
                )
                .trim_end()
                .to_string(),
                Some(out),
            )
        }
    }
}

fn tail(text: &str, lines: usize) -> String {
    let all: Vec<&str> = text.lines().collect();
    all[all.len().saturating_sub(lines)..].join("\n")
}

/// nextest's libtest-json lines.
pub fn parse_nextest(stdout: &str) -> Report {
    let mut report = Report::default();
    let mut saw_suite = false;
    for line in stdout.lines() {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        match (v["type"].as_str(), v["event"].as_str()) {
            (Some("test"), Some("ok")) => report.passed += 1,
            (Some("test"), Some("ignored")) => report.ignored += 1,
            (Some("test"), Some("failed")) => {
                report.failed += 1;
                let full = v["name"].as_str().unwrap_or("?");
                // `crate::binary$path::to::test` → `path::to::test`
                let name = full.split_once('$').map_or(full, |(_, t)| t).to_string();
                report.failures.push(Failure {
                    name,
                    message: v["stdout"].as_str().unwrap_or_default().trim().to_string(),
                    file: None,
                });
            }
            (Some("suite"), _) => saw_suite = true,
            _ => {}
        }
    }
    if !saw_suite {
        report.error = Some("no test results: it may not have compiled (see the log)".into());
    }
    report
}

/// `cargo test`'s text output.
pub fn parse_cargo(output: &str) -> Report {
    let mut report = Report::default();
    let mut saw_result = false;
    for line in output.lines() {
        if let Some(rest) = line.strip_prefix("test result: ") {
            saw_result = true;
            for part in rest.split(';') {
                let words: Vec<&str> = part.split_whitespace().collect();
                let n = words
                    .iter()
                    .find_map(|w| w.trim_end_matches('.').parse::<usize>().ok());
                let what = words.last().copied().unwrap_or_default();
                match (n, what) {
                    (Some(n), "passed") => report.passed += n,
                    (Some(n), "failed") => report.failed += n,
                    (Some(n), "ignored") => report.ignored += n,
                    _ => {}
                }
            }
        }
    }
    // `---- name stdout ----` blocks, up to the next block or `failures:`.
    let mut current: Option<(String, Vec<&str>)> = None;
    for line in output.lines() {
        if let Some(name) = line
            .strip_prefix("---- ")
            .and_then(|l| l.strip_suffix(" stdout ----"))
        {
            if let Some((n, body)) = current.take() {
                report.failures.push(Failure {
                    name: n,
                    message: body.join("\n").trim().to_string(),
                    file: None,
                });
            }
            current = Some((name.to_string(), Vec::new()));
        } else if line == "failures:" {
            if let Some((n, body)) = current.take() {
                report.failures.push(Failure {
                    name: n,
                    message: body.join("\n").trim().to_string(),
                    file: None,
                });
            }
        } else if let Some((_, body)) = &mut current {
            body.push(line);
        }
    }
    if !saw_result {
        report.error = Some(tail(output, 12));
    }
    report
}

/// Vitest's JSON report.
pub fn parse_vitest(json: &str, repo: &Path) -> Report {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(json) else {
        return Report {
            error: Some("no vitest results (see the log)".into()),
            ..Default::default()
        };
    };
    let mut report = Report::default();
    for file in v["testResults"].as_array().into_iter().flatten() {
        let path = file["name"].as_str().unwrap_or_default();
        let rel = Path::new(path)
            .strip_prefix(repo)
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|_| path.to_string());
        for test in file["assertionResults"].as_array().into_iter().flatten() {
            match test["status"].as_str() {
                Some("passed") => report.passed += 1,
                Some("failed") => {
                    report.failed += 1;
                    let message = test["failureMessages"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(|m| m.as_str())
                        .collect::<Vec<_>>()
                        .join("\n");
                    report.failures.push(Failure {
                        name: test["fullName"].as_str().unwrap_or("?").to_string(),
                        message: message.trim().to_string(),
                        file: Some(rel.clone()),
                    });
                }
                Some(_) => report.ignored += 1,
                None => {}
            }
        }
        // A file that failed to load has no assertions, only a message.
        if file["status"] == "failed"
            && file["assertionResults"]
                .as_array()
                .is_none_or(|a| a.is_empty())
        {
            report.failed += 1;
            report.failures.push(Failure {
                name: format!("{rel} (did not run)"),
                message: file["message"]
                    .as_str()
                    .unwrap_or_default()
                    .trim()
                    .to_string(),
                file: Some(rel.clone()),
            });
        }
    }
    report
}

/// The first `relative/path.ext:line[:col]` in text.
pub fn location_in(text: &str) -> Option<(String, u32)> {
    for word in text.split(|c: char| c.is_whitespace() || c == '(' || c == ')' || c == ',') {
        let word = word.trim_end_matches(':');
        let mut parts = word.rsplitn(3, ':');
        let (last, mid, first) = (parts.next(), parts.next(), parts.next());
        let (path, line) = match (first, mid, last) {
            (Some(p), Some(l), Some(_col)) if l.chars().all(|c| c.is_ascii_digit()) => (p, l),
            (None, Some(p), Some(l)) if l.chars().all(|c| c.is_ascii_digit()) => (p, l),
            _ => continue,
        };
        let has_ext = Path::new(path).extension().is_some();
        if has_ext && !path.contains("://") && !path.is_empty() {
            if let Ok(line) = line.parse() {
                return Some((path.trim_start_matches("file://").to_string(), line));
            }
        }
    }
    None
}

/// Runs a command in the repo and parses what it produced.
pub fn run(runner: Runner, repo: &Path, only: &[Failure]) -> Report {
    let (cmd, json_out) = command(runner, repo, only);
    let output = Command::new(crate::supervisor::shell())
        .args(["-lc", &cmd])
        .current_dir(repo)
        .output();
    let output = match output {
        Ok(o) => o,
        Err(e) => {
            return Report {
                error: Some(e.to_string()),
                ..Default::default()
            };
        }
    };
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let mut report = match runner {
        Runner::Nextest => parse_nextest(&stdout),
        Runner::Cargo => parse_cargo(&format!("{stdout}\n{stderr}")),
        Runner::Vitest => {
            let json = json_out
                .as_ref()
                .and_then(|p| std::fs::read_to_string(p).ok())
                .unwrap_or_default();
            if let Some(p) = &json_out {
                let _ = std::fs::remove_file(p);
            }
            parse_vitest(&json, repo)
        }
    };
    if report.error.is_some() && report.passed + report.failed == 0 {
        report.error = Some(tail(&format!("{stdout}\n{stderr}"), 15));
    }
    report
}

/// What an agent is asked when tests fail.
pub fn fix_prompt(runner: Runner, repo_name: &str, report: &Report) -> String {
    let failures: Vec<String> = report
        .failures
        .iter()
        .take(15)
        .map(|f| {
            let message: String = f.message.lines().take(25).collect::<Vec<_>>().join("\n");
            format!("### {}\n\n```\n{message}\n```", f.name)
        })
        .collect();
    let (rerun, _) = command(runner, Path::new("."), &report.failures);
    format!(
        "{} test(s) fail in {repo_name} ({}). Find the cause and fix it: fix the code when the test is right, the test only when it is wrong (say which and why).\n\n{}\n\nRerun just these with:\n\n    {rerun}\n\nThen run the whole suite. Commit the fix on the current branch; do not push.",
        report.failed,
        runner.label(),
        failures.join("\n\n"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const NEXTEST: &str = r#"{"type":"suite","event":"started","test_count":2}
{"type":"test","event":"started","name":"ntest::ntest$tests::ok"}
{"type":"test","event":"ok","name":"ntest::ntest$tests::ok","exec_time":0.014}
{"type":"test","event":"failed","name":"ntest::ntest$tests::bad","exec_time":0.01,"stdout":"\nthread 'tests::bad' panicked at src/lib.rs:7:16:\nassertion `left == right` failed: math\n  left: 2\n right: 3\n"}
{"type":"suite","event":"failed","passed":1,"failed":1,"ignored":0,"measured":0,"filtered_out":0,"exec_time":0.02}"#;

    const CARGO: &str = "test tests::ok ... ok\ntest tests::bad ... FAILED\n\nfailures:\n\n---- tests::bad stdout ----\n\nthread 'tests::bad' panicked at src/lib.rs:7:16:\nassertion `left == right` failed: math\n\n\nfailures:\n    tests::bad\n\ntest result: FAILED. 1 passed; 1 failed; 2 ignored; 0 measured; 0 filtered out; finished in 0.00s\n";

    #[test]
    fn reads_nextest_json() {
        // Given / When
        let report = parse_nextest(NEXTEST);

        // Then
        assert_eq!((report.passed, report.failed), (1, 1));
        assert_eq!(report.failures[0].name, "tests::bad");
        assert!(report.failures[0].message.contains("math"));
        assert_eq!(
            report.failures[0].location(),
            Some(("src/lib.rs".into(), 7))
        );
        assert_eq!(report.error, None);
        assert!(
            parse_nextest("error[E0425]: oops").error.is_some(),
            "no suite: it did not compile"
        );
    }

    #[test]
    fn reads_cargo_test_text() {
        // Given / When
        let report = parse_cargo(CARGO);

        // Then
        assert_eq!((report.passed, report.failed, report.ignored), (1, 1, 2));
        assert_eq!(report.failures.len(), 1);
        assert_eq!(report.failures[0].name, "tests::bad");
        assert!(
            report.failures[0]
                .message
                .starts_with("thread 'tests::bad' panicked")
        );
        assert!(parse_cargo("error: could not compile").error.is_some());
    }

    #[test]
    fn reads_vitest_json() {
        // Given
        let json = r#"{"numFailedTests":1,"testResults":[
          {"name":"/r/src/a.test.ts","status":"failed","assertionResults":[
            {"fullName":"math adds","status":"passed","failureMessages":[]},
            {"fullName":"math subtracts","status":"failed","failureMessages":["AssertionError: expected 1 to be 2\n    at /r/src/a.test.ts:9:17"]},
            {"fullName":"later","status":"skipped","failureMessages":[]}]},
          {"name":"/r/src/broken.test.ts","status":"failed","message":"SyntaxError: x","assertionResults":[]}]}"#;

        // When
        let report = parse_vitest(json, Path::new("/r"));

        // Then
        assert_eq!((report.passed, report.failed, report.ignored), (1, 2, 1));
        assert_eq!(report.failures[0].file.as_deref(), Some("src/a.test.ts"));
        assert_eq!(
            report.failures[0].location(),
            Some(("/r/src/a.test.ts".into(), 9))
        );
        assert_eq!(report.failures[1].name, "src/broken.test.ts (did not run)");
    }

    #[test]
    fn rerun_commands_take_only_the_failures() {
        // Given
        let failure = |name: &str, file: Option<&str>| Failure {
            name: name.into(),
            message: String::new(),
            file: file.map(str::to_string),
        };

        // When
        let (nextest, _) = command(
            Runner::Nextest,
            Path::new("/r"),
            &[failure("tests::bad", None)],
        );
        let (cargo_one, _) = command(
            Runner::Cargo,
            Path::new("/r"),
            &[failure("tests::bad", None)],
        );
        let (cargo_all, _) = command(Runner::Cargo, Path::new("/r"), &[]);
        let (vitest, out) = command(
            Runner::Vitest,
            Path::new("/r"),
            &[
                failure("a", Some("src/a.test.ts")),
                failure("b", Some("src/a.test.ts")),
            ],
        );

        // Then
        assert!(
            nextest.ends_with("--message-format libtest-json 'tests::bad'"),
            "{nextest}"
        );
        assert_eq!(cargo_one, "cargo test --no-fail-fast 'tests::bad'");
        assert_eq!(cargo_all, "cargo test --no-fail-fast");
        assert!(
            vitest.starts_with("npx vitest run --reporter=json --outputFile="),
            "{vitest}"
        );
        assert!(
            vitest.ends_with(" 'src/a.test.ts'"),
            "one file once: {vitest}"
        );
        assert!(out.is_some());
    }

    #[test]
    fn finds_locations() {
        // Given / When / Then
        assert_eq!(
            location_in("panicked at src/lib.rs:7:16:"),
            Some(("src/lib.rs".into(), 7))
        );
        assert_eq!(
            location_in("at foo (/r/a.ts:3:1)"),
            Some(("/r/a.ts".into(), 3))
        );
        assert_eq!(location_in("see https://x.dev:443/a"), None);
        assert_eq!(location_in("nothing here"), None);
    }

    /// A real crate through cargo test and (when installed) nextest.
    #[test]
    fn runs_a_real_crate() {
        // Given: one passing and one failing test
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname = \"probe\"\nversion = \"0.1.0\"\nedition = \"2021\"\n[workspace]\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("src/lib.rs"),
            "#[cfg(test)]\nmod tests {\n    #[test]\n    fn ok() {}\n    #[test]\n    fn bad() { assert_eq!(1, 2, \"nope\"); }\n}\n",
        )
        .unwrap();
        // Its own target dir: never the shared one.
        // SAFETY: only this test reads CARGO_TARGET_DIR in this process.
        unsafe { std::env::set_var("CARGO_TARGET_DIR", dir.path().join("target")) };

        // When
        let runners = detect(dir.path());
        let cargo = run(Runner::Cargo, dir.path(), &[]);
        let nextest = runners
            .contains(&Runner::Nextest)
            .then(|| run(Runner::Nextest, dir.path(), &[]));

        // Then
        assert_eq!((cargo.passed, cargo.failed), (1, 1), "{cargo:?}");
        assert_eq!(cargo.failures[0].name, "tests::bad");
        assert_eq!(
            cargo.failures[0].location().map(|(p, _)| p),
            Some("src/lib.rs".into())
        );
        if let Some(nextest) = nextest {
            assert_eq!((nextest.passed, nextest.failed), (1, 1), "{nextest:?}");
            let rerun = run(Runner::Nextest, dir.path(), &nextest.failures);
            assert_eq!(
                (rerun.passed, rerun.failed),
                (0, 1),
                "only the failure reruns"
            );
        }
    }
}
