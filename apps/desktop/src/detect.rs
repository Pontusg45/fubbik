//! Guess a starter services file from what a repo already has: `package.json`
//! dev scripts and database services in a compose file.

use std::path::Path;

use crate::services::{DatabaseSpec, ProcessSpec, ServicesFile};

const DATABASE_IMAGES: [(&str, &str, &str, u16); 5] = [
    ("postgres", "PostgreSQL", "postgres", 5432),
    ("mysql", "MySQL", "mysql", 3306),
    ("mariadb", "MariaDB", "mysql", 3306),
    ("redis", "Redis", "redis", 6379),
    ("mongo", "MongoDB", "mongodb", 27017),
];

pub fn detect(repo: &Path) -> ServicesFile {
    ServicesFile {
        process: detect_scripts(repo),
        database: detect_compose(repo),
    }
}

fn package_manager(repo: &Path) -> &'static str {
    let has = |name: &str| repo.join(name).exists();
    if has("pnpm-lock.yaml") {
        "pnpm"
    } else if has("bun.lockb") || has("bun.lock") {
        "bun"
    } else if has("yarn.lock") {
        "yarn"
    } else {
        "npm"
    }
}

fn detect_scripts(repo: &Path) -> Vec<ProcessSpec> {
    let Ok(text) = std::fs::read_to_string(repo.join("package.json")) else {
        return Vec::new();
    };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else {
        return Vec::new();
    };
    let Some(scripts) = json.get("scripts").and_then(|s| s.as_object()) else {
        return Vec::new();
    };
    let pm = package_manager(repo);
    scripts
        .keys()
        .filter(|name| *name == "dev" || *name == "start" || name.starts_with("dev:"))
        .map(|name| ProcessSpec {
            name: name.replace(':', "-"),
            command: format!("{pm} run {name}"),
            cwd: None,
            port: None,
            health_url: None,
            report: None,
        })
        .collect()
}

fn detect_compose(repo: &Path) -> Vec<DatabaseSpec> {
    let Some(text) = [
        "docker-compose.yml",
        "docker-compose.yaml",
        "compose.yml",
        "compose.yaml",
    ]
    .iter()
    .find_map(|name| std::fs::read_to_string(repo.join(name)).ok()) else {
        return Vec::new();
    };
    compose_services(&text)
        .into_iter()
        .filter_map(|service| {
            let image = service.image.to_lowercase();
            let (_, engine, scheme, default_port) = DATABASE_IMAGES
                .iter()
                .find(|(needle, ..)| image.contains(needle))?;
            let port = service.host_port.unwrap_or(*default_port);
            Some(DatabaseSpec {
                name: service.name.clone(),
                url: format!("{scheme}://localhost:{port}"),
                engine: Some(engine.to_string()),
                compose_service: Some(service.name),
                start: None,
                stop: None,
                migrate: None,
                seed: None,
                studio: None,
            })
        })
        .collect()
}

#[derive(Debug, Default)]
struct ComposeService {
    name: String,
    image: String,
    host_port: Option<u16>,
}

/// A line scanner, not a YAML parser: enough for `services:` entries with
/// `image:` and `ports:` (`- "5432:5432"`).
fn compose_services(text: &str) -> Vec<ComposeService> {
    let mut services = Vec::new();
    let mut in_services = false;
    let mut service_indent = None;
    let mut in_ports = false;
    for line in text.lines() {
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let indent = line.len() - trimmed.len();
        if indent == 0 {
            in_services = trimmed.starts_with("services:");
            service_indent = None;
            continue;
        }
        if !in_services {
            continue;
        }
        let indent_of_services = *service_indent.get_or_insert(indent);
        if indent == indent_of_services {
            if let Some(name) = trimmed.strip_suffix(':') {
                services.push(ComposeService {
                    name: name.to_string(),
                    ..Default::default()
                });
            }
            in_ports = false;
            continue;
        }
        let Some(service) = services.last_mut() else {
            continue;
        };
        if let Some(image) = trimmed.strip_prefix("image:") {
            service.image = image.trim().trim_matches('"').to_string();
            in_ports = false;
        } else if trimmed.starts_with("ports:") {
            in_ports = true;
        } else if in_ports && trimmed.starts_with('-') {
            let mapping = trimmed.trim_start_matches('-').trim().trim_matches('"');
            let host = mapping
                .rsplit_once(':')
                .map(|(host, _)| host)
                .unwrap_or(mapping);
            let host = host.rsplit(':').next().unwrap_or(host);
            if service.host_port.is_none() {
                service.host_port = host.parse().ok();
            }
        } else if !trimmed.starts_with('-') {
            in_ports = false;
        }
    }
    services
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_pnpm_dev_scripts() {
        // Given
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("pnpm-lock.yaml"), "").unwrap();
        std::fs::write(
            dir.path().join("package.json"),
            r#"{"scripts":{"dev":"vite","dev:web":"vite","build":"vite build"}}"#,
        )
        .unwrap();

        // When
        let file = detect(dir.path());

        // Then
        let commands: Vec<_> = file.process.iter().map(|p| p.command.as_str()).collect();
        assert_eq!(commands, ["pnpm run dev", "pnpm run dev:web"]);
        assert_eq!(file.process[1].name, "dev-web");
    }

    #[test]
    fn detects_compose_postgres_with_port() {
        // Given
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("docker-compose.yml"),
            "services:\n  api:\n    image: node:22\n    ports:\n      - \"3000:3000\"\n  db:\n    image: postgres:18\n    ports:\n      - \"127.0.0.1:5433:5432\"\nvolumes:\n  data:\n",
        )
        .unwrap();

        // When
        let file = detect(dir.path());

        // Then
        assert_eq!(file.database.len(), 1);
        assert_eq!(file.database[0].name, "db");
        assert_eq!(file.database[0].url, "postgres://localhost:5433");
        assert_eq!(file.database[0].compose_service.as_deref(), Some("db"));
    }

    #[test]
    fn empty_repo_detects_nothing() {
        // Given
        let dir = tempfile::tempdir().unwrap();

        // When
        let file = detect(dir.path());

        // Then
        assert_eq!(file, ServicesFile::default());
    }
}
