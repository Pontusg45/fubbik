//! Runs checks over the repository and gathers their results.

use std::io;
use std::path::Path;

use crate::{Check, Violation, source};

/// One check's outcome.
#[derive(Debug)]
pub struct CheckResult {
    pub name: String,
    pub files_checked: usize,
    pub violations: Vec<Violation>,
}

/// The outcome of a run.
#[derive(Debug, Default)]
pub struct Report {
    pub results: Vec<CheckResult>,
}

impl Report {
    pub fn violation_count(&self) -> usize {
        self.results.iter().map(|r| r.violations.len()).sum()
    }

    pub fn passed(&self) -> bool {
        self.violation_count() == 0
    }
}

/// Run each check against the files under `repo_root`.
pub fn run(repo_root: &Path, checks: &[Box<dyn Check>]) -> io::Result<Report> {
    let mut report = Report::default();
    for check in checks {
        let files = source::collect(repo_root, &check.roots(), &check.extensions())?;
        let violations = files.iter().flat_map(|f| check.check(f)).collect();
        report.results.push(CheckResult {
            name: check.name().to_string(),
            files_checked: files.len(),
            violations,
        });
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::TextFile;

    struct NoTodo;

    impl Check for NoTodo {
        fn name(&self) -> &str {
            "no-todo"
        }
        fn description(&self) -> &str {
            "test check"
        }
        fn roots(&self) -> Vec<&str> {
            vec!["src"]
        }
        fn extensions(&self) -> Vec<&str> {
            vec![".ts"]
        }
        fn check(&self, file: &TextFile) -> Vec<Violation> {
            file.text
                .lines()
                .enumerate()
                .filter(|(_, l)| l.contains("TODO"))
                .map(|(i, _)| Violation {
                    file: file.path.clone(),
                    line: i + 1,
                    message: "TODO left in".into(),
                })
                .collect()
        }
    }

    fn temp_repo(name: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("review-harness-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("src/node_modules")).unwrap();
        dir
    }

    #[test]
    fn reports_violations_with_line_numbers_and_skips_ignored_files() {
        // Given
        let repo = temp_repo("violations");
        fs::write(repo.join("src/a.ts"), "ok\n// TODO fix\n").unwrap();
        fs::write(repo.join("src/routes.gen.ts"), "// TODO generated\n").unwrap();
        fs::write(repo.join("src/node_modules/dep.ts"), "// TODO vendored\n").unwrap();
        fs::write(repo.join("src/b.rs"), "// TODO other language\n").unwrap();
        let checks: Vec<Box<dyn Check>> = vec![Box::new(NoTodo)];
        // When
        let report = run(&repo, &checks).unwrap();
        // Then
        assert_eq!(report.results[0].files_checked, 1);
        assert_eq!(
            report.results[0].violations,
            vec![Violation {
                file: "src/a.ts".into(),
                line: 2,
                message: "TODO left in".into(),
            }]
        );
        assert!(!report.passed());
        fs::remove_dir_all(repo).unwrap();
    }

    #[test]
    fn passes_when_no_check_finds_anything() {
        // Given
        let repo = temp_repo("clean");
        fs::write(repo.join("src/a.ts"), "clean\n").unwrap();
        let checks: Vec<Box<dyn Check>> = vec![Box::new(NoTodo)];
        // When
        let report = run(&repo, &checks).unwrap();
        // Then
        assert!(report.passed());
        fs::remove_dir_all(repo).unwrap();
    }
}
