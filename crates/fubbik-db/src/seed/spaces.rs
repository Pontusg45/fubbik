use std::path::Path;
use std::process::Command;

use anyhow::Result;
use sqlx::{Postgres, Transaction};

pub const DEV_USER_ID: &str = "dev-user";

/// Resolves where the seed code space lives: `(local_paths, remote_url)`.
///
/// Env overrides win (blank values are ignored); otherwise the path is the
/// current directory and the remote is whatever `git_remote` reports for it.
fn seed_space_location(
    env_path: Option<String>,
    env_remote: Option<String>,
    cwd: &Path,
    git_remote: impl Fn(&Path) -> Option<String>,
) -> (Vec<String>, Option<String>) {
    let non_blank = |v: Option<String>| v.filter(|v| !v.trim().is_empty());
    let path = non_blank(env_path).unwrap_or_else(|| cwd.display().to_string());
    let remote = non_blank(env_remote).or_else(|| git_remote(cwd));
    (vec![path], remote)
}

fn git_origin_url(cwd: &Path) -> Option<String> {
    let out = Command::new("git")
        .args(["remote", "get-url", "origin"])
        .current_dir(cwd)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let url = String::from_utf8(out.stdout).ok()?.trim().to_string();
    (!url.is_empty()).then_some(url)
}

/// Inserts the two legacy code spaces under stable seed-owned IDs.
pub async fn seed_spaces(tx: &mut Transaction<'_, Postgres>) -> Result<usize> {
    // Canonicalise so the default is stable regardless of symlinks; fall back to the raw cwd.
    let cwd = std::env::current_dir()
        .and_then(|d| d.canonicalize())
        .unwrap_or_default();
    let (paths, main_remote) = seed_space_location(
        std::env::var("FUBBIK_SEED_SPACE_PATH").ok(),
        std::env::var("FUBBIK_SEED_REMOTE_URL").ok(),
        &cwd,
        git_origin_url,
    );
    for (seed_id, name, remote_url, local_paths) in [
        (
            "seed-codebase-fubbik",
            "fubbik",
            main_remote,
            serde_json::json!(paths),
        ),
        (
            "seed-codebase-docs",
            "fubbik-docs",
            None,
            serde_json::json!([]),
        ),
    ] {
        let id: String =
            sqlx::query_scalar("SELECT id FROM space WHERE user_id = $1 AND name = $2")
                .bind(DEV_USER_ID)
                .bind(name)
                .fetch_optional(&mut **tx)
                .await?
                .unwrap_or_else(|| seed_id.to_string());
        sqlx::query(
            "INSERT INTO space (id, name, kind, user_id) VALUES ($1, $2, 'code', $3) \
             ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.name, kind = EXCLUDED.kind, \
             user_id = EXCLUDED.user_id, updated_at = now()",
        )
        .bind(&id)
        .bind(name)
        .bind(DEV_USER_ID)
        .execute(&mut **tx)
        .await?;
        sqlx::query(
            "INSERT INTO space_code_metadata (space_id, user_id, remote_url, local_paths) \
             VALUES ($1, $2, $3, $4) ON CONFLICT (space_id) DO UPDATE SET \
             remote_url = EXCLUDED.remote_url, local_paths = EXCLUDED.local_paths",
        )
        .bind(&id)
        .bind(DEV_USER_ID)
        .bind(remote_url)
        .bind(local_paths)
        .execute(&mut **tx)
        .await?;
    }
    Ok(2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seed_space_uses_env_overrides() {
        let (paths, remote) = seed_space_location(
            Some("/srv/code/mine".into()),
            Some("git@example.test:me/mine.git".into()),
            Path::new("/ignored"),
            |_| Some("git@example.test:other/other.git".into()),
        );
        assert_eq!(paths, vec!["/srv/code/mine".to_string()]);
        assert_eq!(remote.as_deref(), Some("git@example.test:me/mine.git"));
    }

    #[test]
    fn seed_space_defaults_to_cwd_and_git_origin() {
        let (paths, remote) = seed_space_location(None, None, Path::new("/ignored/repo"), |cwd| {
            assert_eq!(cwd, Path::new("/ignored/repo"));
            Some("git@example.test:me/repo.git".into())
        });
        assert_eq!(paths, vec!["/ignored/repo".to_string()]);
        assert_eq!(remote.as_deref(), Some("git@example.test:me/repo.git"));
    }

    #[test]
    fn seed_space_has_no_remote_when_git_fails_and_ignores_blank_env() {
        let (paths, remote) = seed_space_location(
            Some("  ".into()),
            Some("".into()),
            Path::new("/ignored/repo"),
            |_| None,
        );
        assert_eq!(paths, vec!["/ignored/repo".to_string()]);
        assert_eq!(remote, None);
    }
}
