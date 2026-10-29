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
    // An empty cwd means the working directory could not be determined: no default path, no git call.
    let has_cwd = !cwd.as_os_str().is_empty();
    let path = non_blank(env_path).or_else(|| has_cwd.then(|| cwd.display().to_string()));
    let remote = non_blank(env_remote).or_else(|| if has_cwd { git_remote(cwd) } else { None });
    (path.into_iter().collect(), remote)
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
        // Given both env overrides and a cwd whose git origin differs
        let env_path = Some("/srv/code/mine".to_string());
        let env_remote = Some("git@example.test:me/mine.git".to_string());
        // When the location is resolved
        let (paths, remote) =
            seed_space_location(env_path, env_remote, Path::new("/ignored"), |_| {
                Some("git@example.test:other/other.git".into())
            });
        // Then the env values win
        assert_eq!(paths, vec!["/srv/code/mine".to_string()]);
        assert_eq!(remote.as_deref(), Some("git@example.test:me/mine.git"));
    }

    #[test]
    fn seed_space_defaults_to_cwd_and_git_origin() {
        // Given no env overrides and a cwd with a git origin
        let cwd = Path::new("/ignored/repo");
        // When the location is resolved
        let (paths, remote) = seed_space_location(None, None, cwd, |dir| {
            assert_eq!(dir, cwd);
            Some("git@example.test:me/repo.git".into())
        });
        // Then the cwd and its origin are used
        assert_eq!(paths, vec!["/ignored/repo".to_string()]);
        assert_eq!(remote.as_deref(), Some("git@example.test:me/repo.git"));
    }

    #[test]
    fn seed_space_has_no_remote_when_git_fails_and_ignores_blank_env() {
        // Given blank env overrides and a git that reports no origin
        let blank = Some("  ".to_string());
        // When the location is resolved
        let (paths, remote) = seed_space_location(
            blank,
            Some(String::new()),
            Path::new("/ignored/repo"),
            |_| None,
        );
        // Then the cwd is used and there is no remote
        assert_eq!(paths, vec!["/ignored/repo".to_string()]);
        assert_eq!(remote, None);
    }

    #[test]
    fn seed_space_has_no_path_and_skips_git_when_cwd_is_unknown() {
        // Given no env overrides and an empty (undeterminable) cwd
        let cwd = Path::new("");
        // When the location is resolved
        let (paths, remote) = seed_space_location(None, None, cwd, |_| {
            panic!("git must not run without a cwd")
        });
        // Then there is no local path and no remote
        assert!(paths.is_empty());
        assert_eq!(remote, None);
    }
}
