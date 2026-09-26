use anyhow::Result;
use sqlx::{Postgres, Transaction};

pub const DEV_USER_ID: &str = "dev-user";

/// Inserts the two legacy code spaces under stable seed-owned IDs.
pub async fn seed_spaces(tx: &mut Transaction<'_, Postgres>) -> Result<usize> {
    for (seed_id, name, remote_url, local_paths) in [
        (
            "seed-codebase-fubbik",
            "fubbik",
            Some("git@github.com:Pontusg45/fubbik.git"),
            serde_json::json!(["/Users/pontus/projects/fubbik"]),
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
