//! Docker (OrbStack) containers per repo, and Docker's disk use: which
//! containers belong to a repo (its compose project's folder, or a name or
//! label starting with the repo's name), their state and ports, and the
//! commands to start, stop, restart and follow them.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Container {
    pub id: String,
    pub name: String,
    pub image: String,
    /// `running`, `exited`, `paused`, `created`…
    pub state: String,
    /// `Up 4 hours`, `Exited (0) 2 days ago`.
    pub status: String,
    pub ports: String,
    pub labels: BTreeMap<String, String>,
}

impl Container {
    pub fn running(&self) -> bool {
        self.state == "running"
    }

    pub fn compose_project(&self) -> Option<&str> {
        self.labels
            .get("com.docker.compose.project")
            .map(String::as_str)
    }

    /// The compose service (`postgres`), else the container name.
    pub fn service(&self) -> &str {
        self.labels
            .get("com.docker.compose.service")
            .map_or(self.name.as_str(), String::as_str)
    }

    /// Whether it belongs to the repo at `path` named `name`: its compose
    /// project's folder is in the repo, or its name or a label key starts
    /// with the repo's name (`fubbik-rust-test-db`, `fubbik.rust-test-profile`).
    pub fn belongs_to(&self, path: &Path, name: &str) -> bool {
        if let Some(dir) = self.labels.get("com.docker.compose.project.working_dir") {
            return Path::new(dir).starts_with(path);
        }
        let lower = name.to_lowercase();
        self.name.to_lowercase().starts_with(&format!("{lower}-"))
            || self.name.to_lowercase().starts_with(&format!("{lower}_"))
            || self
                .labels
                .keys()
                .any(|k| k.to_lowercase().starts_with(&format!("{lower}.")))
    }
}

/// `docker ps -a --format '{{json .}}'`, one container per line.
pub fn parse_ps(text: &str) -> Vec<Container> {
    text.lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .map(|v| {
            let s = |k: &str| v[k].as_str().unwrap_or_default().to_string();
            let labels = s("Labels")
                .split(',')
                .filter_map(|kv| kv.split_once('='))
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect();
            Container {
                id: s("ID"),
                name: s("Names"),
                image: s("Image"),
                state: s("State"),
                status: s("Status"),
                ports: s("Ports"),
                labels,
            }
        })
        .collect()
}

fn docker(args: &[&str]) -> Result<String, String> {
    let output = Command::new("docker")
        .args(args)
        .output()
        .map_err(|e| format!("docker: {e}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Every container, running or not. Err when Docker is not running.
pub fn list() -> Result<Vec<Container>, String> {
    docker(&["ps", "-a", "--format", "{{json .}}"]).map(|t| parse_ps(&t))
}

/// `start`, `stop` or `restart` one container (by name).
pub fn act(action: &str, name: &str) -> Result<(), String> {
    if !matches!(action, "start" | "stop" | "restart") {
        return Err(format!("unknown container action {action}"));
    }
    docker(&[action, name]).map(|_| ())
}

/// The command that follows a container's log (for a terminal tab).
pub fn logs_command(name: &str) -> String {
    format!("docker logs -f --tail 200 '{}'", name.replace('\'', ""))
}

/// One line of `docker system df`: images, containers, volumes, build cache.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiskUse {
    pub kind: String,
    pub count: String,
    pub size: String,
    pub reclaimable: String,
}

pub fn parse_df(text: &str) -> Vec<DiskUse> {
    text.lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .map(|v| {
            let s = |k: &str| v[k].as_str().unwrap_or_default().to_string();
            DiskUse {
                kind: s("Type"),
                count: s("TotalCount"),
                size: s("Size"),
                reclaimable: s("Reclaimable"),
            }
        })
        .collect()
}

/// `docker system df`.
pub fn disk() -> Result<Vec<DiskUse>, String> {
    docker(&["system", "df", "--format", "{{json .}}"]).map(|t| parse_df(&t))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_containers_and_finds_their_repo() {
        // Given: a compose container, a named one, and a stranger
        let text = r#"{"ID":"a1","Names":"vpg-platform-postgres-1","Image":"postgres:17-alpine","State":"running","Status":"Up 4 hours","Ports":"127.0.0.1:5432->5432/tcp","Labels":"com.docker.compose.project=vpg-platform,com.docker.compose.project.working_dir=/Users/p/projects/vpg-platform,com.docker.compose.service=postgres"}
{"ID":"b2","Names":"fubbik-rust-test-db","Image":"fubbik-postgres:pg18","State":"running","Status":"Up 4 hours","Ports":"127.0.0.1:5434->5432/tcp","Labels":"fubbik.rust-test-profile=age"}
{"ID":"c3","Names":"redis","Image":"redis:7","State":"exited","Status":"Exited (0) 2 days ago","Ports":"","Labels":""}
not json"#;

        // When
        let all = parse_ps(text);
        let platform = Path::new("/Users/p/projects/vpg-platform");
        let fubbik = Path::new("/Users/p/projects/fubbik");

        // Then
        assert_eq!(all.len(), 3);
        assert_eq!(all[0].service(), "postgres");
        assert_eq!(all[0].compose_project(), Some("vpg-platform"));
        assert!(all[0].belongs_to(platform, "vpg-platform"));
        assert!(
            !all[0].belongs_to(fubbik, "fubbik"),
            "a compose folder decides"
        );
        assert!(all[1].belongs_to(fubbik, "fubbik"));
        assert_eq!(all[1].service(), "fubbik-rust-test-db");
        assert!(!all[2].belongs_to(fubbik, "fubbik"));
        assert!(!all[2].running());
    }

    #[test]
    fn reads_disk_use() {
        // Given
        let text = r#"{"Active":"3","Reclaimable":"4.1GB (62%)","Size":"6.6GB","TotalCount":"12","Type":"Images"}
{"Active":"0","Reclaimable":"1.2GB","Size":"1.2GB","TotalCount":"0","Type":"Build Cache"}"#;

        // When
        let disk = parse_df(text);

        // Then
        assert_eq!(disk.len(), 2);
        assert_eq!(disk[0].kind, "Images");
        assert_eq!(disk[0].reclaimable, "4.1GB (62%)");
        assert_eq!(logs_command("a'b"), "docker logs -f --tail 200 'ab'");
    }
}
