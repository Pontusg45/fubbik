//! A repo's services file, `fubbik.services.toml` at the repo root: what runs
//! for the repo and which databases it uses.
//!
//! ```toml
//! [[process]]
//! name = "api"
//! command = "just dev-rust"
//! port = 3000
//! health_url = "http://127.0.0.1:3000/api/health"
//!
//! [[database]]
//! name = "postgres"
//! url = "postgres://localhost:5432/fubbik"
//! compose_service = "db"
//! migrate = "pnpm seed --reset none --only core"
//! ```

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const FILE_NAME: &str = "fubbik.services.toml";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServicesFile {
    #[serde(default)]
    pub process: Vec<ProcessSpec>,
    #[serde(default)]
    pub database: Vec<DatabaseSpec>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProcessSpec {
    pub name: String,
    pub command: String,
    /// Relative to the repo root.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub health_url: Option<String>,
    /// For test commands: where a finished run's pass/fail is reported.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report: Option<TestReport>,
}

/// `[process.report]`: when the process exits on its own, exit code 0 is a
/// pass and anything else a fail.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TestReport {
    /// A requirement id: set to `passing` / `failing`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requirement: Option<String>,
    /// A matrix id and cell id: a test result is recorded on the cell.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matrix: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cell: Option<String>,
    /// The test result's `testRef`; defaults to the command.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub test_ref: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DatabaseSpec {
    pub name: String,
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine: Option<String>,
    /// A docker compose service: used for status, and for start/stop when
    /// `start`/`stop` are not given.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compose_service: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub migrate: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub studio: Option<String>,
}

impl DatabaseSpec {
    pub fn start_command(&self) -> Option<String> {
        self.start.clone().or_else(|| {
            self.compose_service
                .as_ref()
                .map(|service| format!("docker compose up -d {service}"))
        })
    }

    pub fn stop_command(&self) -> Option<String> {
        self.stop.clone().or_else(|| {
            self.compose_service
                .as_ref()
                .map(|service| format!("docker compose stop {service}"))
        })
    }
}

impl ProcessSpec {
    pub fn cwd(&self, repo: &Path) -> PathBuf {
        match &self.cwd {
            Some(cwd) => repo.join(cwd),
            None => repo.to_path_buf(),
        }
    }
}

pub fn path(repo: &Path) -> PathBuf {
    repo.join(FILE_NAME)
}

impl ServicesFile {
    pub fn parse(text: &str) -> Result<ServicesFile, String> {
        toml::from_str(text).map_err(|e| e.message().to_string())
    }

    /// `Ok(None)` when the repo has no services file yet.
    pub fn load(repo: &Path) -> Result<Option<ServicesFile>, String> {
        match std::fs::read_to_string(path(repo)) {
            Ok(text) => Self::parse(&text)
                .map(Some)
                .map_err(|e| format!("{FILE_NAME}: {e}")),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(format!("{FILE_NAME}: {e}")),
        }
    }

    pub fn save(&self, repo: &Path) -> Result<(), String> {
        let text = toml::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(path(repo), text).map_err(|e| format!("{FILE_NAME}: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_processes_and_databases() {
        // Given
        let text = r#"
            [[process]]
            name = "api"
            command = "just dev-rust"
            port = 3000

            [[database]]
            name = "postgres"
            url = "postgres://localhost:5432/fubbik"
            compose_service = "db"
        "#;

        // When
        let file = ServicesFile::parse(text).unwrap();

        // Then
        assert_eq!(file.process[0].name, "api");
        assert_eq!(file.process[0].port, Some(3000));
        assert_eq!(
            file.database[0].start_command().as_deref(),
            Some("docker compose up -d db")
        );
    }

    #[test]
    fn parses_test_report() {
        // Given
        let text = "[[process]]\nname = \"tests\"\ncommand = \"just rust-test-fast\"\n[process.report]\nrequirement = \"r1\"\nmatrix = \"m1\"\ncell = \"c1\"\n";

        // When
        let file = ServicesFile::parse(text).unwrap();

        // Then
        let report = file.process[0].report.as_ref().unwrap();
        assert_eq!(report.requirement.as_deref(), Some("r1"));
        assert_eq!(
            (report.matrix.as_deref(), report.cell.as_deref()),
            (Some("m1"), Some("c1"))
        );
    }

    #[test]
    fn rejects_unknown_keys() {
        // Given
        let text = "[[process]]\nname = \"api\"\ncommand = \"x\"\nprot = 3000\n";

        // When
        let result = ServicesFile::parse(text);

        // Then
        assert!(result.unwrap_err().contains("prot"));
    }

    #[test]
    fn missing_services_file_is_none() {
        // Given
        let dir = tempfile::tempdir().unwrap();

        // When
        let loaded = ServicesFile::load(dir.path()).unwrap();

        // Then
        assert!(loaded.is_none());
    }

    #[test]
    fn save_then_load_round_trips() {
        // Given
        let dir = tempfile::tempdir().unwrap();
        let file =
            ServicesFile::parse("[[process]]\nname = \"web\"\ncommand = \"pnpm dev\"\n").unwrap();

        // When
        file.save(dir.path()).unwrap();
        let loaded = ServicesFile::load(dir.path()).unwrap();

        // Then
        assert_eq!(loaded, Some(file));
    }
}
