//! A repo's PostgreSQL databases (the `[[database]]` URLs in its services
//! file): size, connections and version, sqlx migration drift (applied vs
//! the repo's migration files), `psql`, and snapshots (`pg_dump`) with
//! restore. The client tools come from PATH, Postgres.app or Homebrew's
//! libpq.

use std::path::{Path, PathBuf};
use std::process::Command;

/// `psql`, `pg_dump`, `pg_restore`: on PATH, else Postgres.app's, else
/// Homebrew's keg-only libpq.
pub fn tool(name: &str) -> Option<PathBuf> {
    let on_path = std::env::var_os("PATH").and_then(|path| {
        std::env::split_paths(&path)
            .map(|d| d.join(name))
            .find(|p| p.is_file())
    });
    on_path.or_else(|| {
        [
            "/Applications/Postgres.app/Contents/Versions/latest/bin",
            "/opt/homebrew/opt/libpq/bin",
            "/usr/local/opt/libpq/bin",
        ]
        .iter()
        .map(|d| Path::new(d).join(name))
        .find(|p| p.is_file())
    })
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DbInfo {
    pub version: String,
    pub size_bytes: u64,
    pub connections: u64,
    /// sqlx migrations applied (versions), None when the database has no
    /// `_sqlx_migrations` table.
    pub applied: Option<Vec<i64>>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Drift {
    /// Files in the repo not applied yet.
    pub pending: Vec<String>,
    /// Applied versions with no file in the repo (another branch, a newer
    /// checkout, or a renamed file).
    pub unknown: Vec<i64>,
}

fn psql(url: &str, sql: &str) -> Result<String, String> {
    let psql =
        tool("psql").ok_or("psql not found (install Postgres.app or `brew install libpq`)")?;
    let output = Command::new(psql)
        .args(["-X", "-At", "-F", "|", "-d", url, "-c", sql])
        .env("PGCONNECT_TIMEOUT", "3")
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Version, size, connections and applied sqlx migrations.
pub fn inspect(url: &str) -> Result<DbInfo, String> {
    let row = psql(
        url,
        "select current_setting('server_version'), pg_database_size(current_database()), \
         (select count(*) from pg_stat_activity where datname = current_database()), \
         to_regclass('_sqlx_migrations') is not null",
    )?;
    let mut info = parse_info(&row).ok_or_else(|| format!("unexpected answer: {row}"))?;
    if row.ends_with("|t") {
        let versions = psql(
            url,
            "select version from _sqlx_migrations where success order by version",
        )?;
        info.applied = Some(
            versions
                .lines()
                .filter_map(|l| l.trim().parse().ok())
                .collect(),
        );
    }
    Ok(info)
}

/// `18.1|123456|3|t`.
pub fn parse_info(row: &str) -> Option<DbInfo> {
    let mut f = row.split('|');
    Some(DbInfo {
        version: f.next()?.to_string(),
        size_bytes: f.next()?.parse().ok()?,
        connections: f.next()?.parse().ok()?,
        applied: None,
    })
}

/// The repo's sqlx-style migration files (`NNNN_name.sql` in folders named
/// `migrations`), as (version, file name), each folder its own set.
pub fn migration_sets(repo: &Path) -> Vec<(PathBuf, Vec<(i64, String)>)> {
    fn walk(dir: &Path, depth: usize, out: &mut Vec<(PathBuf, Vec<(i64, String)>)>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            if !path.is_dir()
                || name.starts_with('.')
                || matches!(name.as_str(), "node_modules" | "target" | "dist")
            {
                continue;
            }
            if name == "migrations" {
                let mut set = Vec::new();
                for file in std::fs::read_dir(&path).into_iter().flatten().flatten() {
                    let file_name = file.file_name().to_string_lossy().into_owned();
                    let digits: String = file_name
                        .chars()
                        .take_while(|c| c.is_ascii_digit())
                        .collect();
                    if file_name.ends_with(".sql")
                        && !digits.is_empty()
                        && file_name[digits.len()..].starts_with('_')
                        && let Ok(version) = digits.parse()
                    {
                        set.push((version, file_name));
                    }
                }
                if !set.is_empty() {
                    set.sort();
                    out.push((path, set));
                }
            } else if depth < 4 {
                walk(&path, depth + 1, out);
            }
        }
    }
    let mut out = Vec::new();
    walk(repo, 0, &mut out);
    out.sort();
    out
}

/// The set a database follows: the one sharing the most versions with what
/// it has applied (a Drizzle folder next to a sqlx one is not mixed in).
pub fn best_set(applied: &[i64], sets: &[(PathBuf, Vec<(i64, String)>)]) -> Vec<(i64, String)> {
    sets.iter()
        .max_by_key(|(_, files)| {
            (
                files.iter().filter(|(v, _)| applied.contains(v)).count(),
                files.len(),
            )
        })
        .map(|(_, files)| files.clone())
        .unwrap_or_default()
}

pub fn drift(applied: &[i64], files: &[(i64, String)]) -> Drift {
    Drift {
        pending: files
            .iter()
            .filter(|(v, _)| !applied.contains(v))
            .map(|(_, f)| f.clone())
            .collect(),
        unknown: applied
            .iter()
            .filter(|v| !files.iter().any(|(f, _)| f == *v))
            .copied()
            .collect(),
    }
}

/// The database name in a URL (`postgresql://h:5432/fubbik?x` → `fubbik`).
pub fn db_name(url: &str) -> String {
    url.rsplit('/')
        .next()
        .unwrap_or("db")
        .split('?')
        .next()
        .unwrap_or("db")
        .to_string()
}

/// Where snapshots go: `~/Library/Application Support/fubbik/snapshots`.
pub fn snapshot_dir() -> PathBuf {
    crate::backend::data_dir().join("snapshots")
}

/// `pg_dump -Fc` of the database into the snapshots folder.
pub fn snapshot(url: &str, stamp: &str) -> Result<PathBuf, String> {
    let dump = tool("pg_dump")
        .ok_or("pg_dump not found (install Postgres.app or `brew install libpq`)")?;
    let dir = snapshot_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let file = dir.join(format!("{}-{stamp}.dump", db_name(url)));
    let output = Command::new(dump)
        .args(["-Fc", "--no-owner", "-d", url, "-f"])
        .arg(&file)
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        let _ = std::fs::remove_file(&file);
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(file)
}

/// This database's snapshots, newest first.
pub fn snapshots(url: &str) -> Vec<PathBuf> {
    let prefix = format!("{}-", db_name(url));
    let mut out: Vec<PathBuf> = std::fs::read_dir(snapshot_dir())
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.extension().is_some_and(|e| e == "dump")
                && p.file_name()
                    .is_some_and(|n| n.to_string_lossy().starts_with(&prefix))
        })
        .collect();
    out.sort();
    out.reverse();
    out
}

fn quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// `psql` on the database (for a terminal tab).
pub fn psql_command(url: &str) -> Option<String> {
    Some(format!(
        "{} {}",
        quote(&tool("psql")?.to_string_lossy()),
        quote(url)
    ))
}

/// Restores a snapshot over the database (for a terminal tab, where its
/// output shows): drops and recreates the objects it contains.
pub fn restore_command(url: &str, file: &Path) -> Option<String> {
    Some(format!(
        "{} --clean --if-exists --no-owner -d {} {}",
        quote(&tool("pg_restore")?.to_string_lossy()),
        quote(url),
        quote(&file.to_string_lossy())
    ))
}

/// `123456789` → `117.7 MB`.
pub fn human_size(bytes: u64) -> String {
    let units = ["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < units.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{size:.1} {}", units[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_info_names_and_sizes() {
        // Given / When / Then
        assert_eq!(
            parse_info("18.1|123456789|3|t"),
            Some(DbInfo {
                version: "18.1".into(),
                size_bytes: 123_456_789,
                connections: 3,
                applied: None
            })
        );
        assert_eq!(parse_info("garbage"), None);
        assert_eq!(
            db_name("postgresql://u:p@localhost:5434/fubbik_rs?sslmode=disable"),
            "fubbik_rs"
        );
        assert_eq!(human_size(123_456_789), "117.7 MB");
        assert_eq!(human_size(512), "512 B");
    }

    #[test]
    fn finds_migrations_and_drift() {
        // Given: crates/db/migrations with three files, one not a migration
        let dir = tempfile::tempdir().unwrap();
        let migrations = dir.path().join("crates/db/migrations");
        std::fs::create_dir_all(&migrations).unwrap();
        for f in [
            "0001_init.sql",
            "0002_more.sql",
            "0004_last.sql",
            "README.md",
            "notes.sql",
        ] {
            std::fs::write(migrations.join(f), "").unwrap();
        }
        std::fs::create_dir_all(dir.path().join("node_modules/x/migrations")).unwrap();
        std::fs::write(dir.path().join("node_modules/x/migrations/0009_no.sql"), "").unwrap();

        let drizzle = dir.path().join("packages/db/migrations");
        std::fs::create_dir_all(&drizzle).unwrap();
        std::fs::write(drizzle.join("0000_baseline.sql"), "").unwrap();

        // When
        let sets = migration_sets(dir.path());
        let files = best_set(&[1, 2, 3], &sets);
        let drift = drift(&[1, 2, 3], &files);

        // Then
        assert_eq!(files.iter().map(|(v, _)| *v).collect::<Vec<_>>(), [1, 2, 4]);
        assert_eq!(drift.pending, ["0004_last.sql"]);
        assert_eq!(drift.unknown, [3]);
    }
}
