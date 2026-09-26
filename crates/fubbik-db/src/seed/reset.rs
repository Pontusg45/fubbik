use anyhow::Result;
use sqlx::{Postgres, Transaction};

use super::spaces::DEV_USER_ID;

/// Mirrors the legacy module reset hooks in reverse dependency order, scoped
/// to the seed user. Tables with cascading children need only their root row.
pub async fn reset_selected(tx: &mut Transaction<'_, Postgres>, modules: &[&str]) -> Result<()> {
    for module in modules.iter().rev() {
        match *module {
            "matrices" => {
                sqlx::query("DELETE FROM behavior_matrix WHERE user_id = $1")
                    .bind(DEV_USER_ID)
                    .execute(&mut **tx)
                    .await?;
            }
            "collections" => {
                sqlx::query("DELETE FROM collection WHERE user_id = $1")
                    .bind(DEV_USER_ID)
                    .execute(&mut **tx)
                    .await?;
            }
            "workspaces" => {
                sqlx::query("DELETE FROM workspace WHERE user_id = $1")
                    .bind(DEV_USER_ID)
                    .execute(&mut **tx)
                    .await?;
            }
            "vocabulary" => {
                sqlx::query("DELETE FROM vocabulary_entry WHERE user_id = $1")
                    .bind(DEV_USER_ID)
                    .execute(&mut **tx)
                    .await?;
            }
            "documents" => {
                sqlx::query("DELETE FROM document WHERE user_id = $1")
                    .bind(DEV_USER_ID)
                    .execute(&mut **tx)
                    .await?;
            }
            "agent-coordination" => {
                sqlx::query(
                    "DELETE FROM agent_run WHERE plan_id IN (SELECT id FROM plan WHERE user_id = $1)",
                )
                .bind(DEV_USER_ID).execute(&mut **tx).await?;
            }
            "plans" => {
                sqlx::query("DELETE FROM plan WHERE user_id = $1")
                    .bind(DEV_USER_ID)
                    .execute(&mut **tx)
                    .await?;
            }
            "requirements" => {
                sqlx::query("DELETE FROM requirement WHERE user_id = $1")
                    .bind(DEV_USER_ID)
                    .execute(&mut **tx)
                    .await?;
            }
            "use-cases" => {
                sqlx::query("DELETE FROM use_case WHERE user_id = $1")
                    .bind(DEV_USER_ID)
                    .execute(&mut **tx)
                    .await?;
            }
            "chunks" => {
                sqlx::query("DELETE FROM chunk WHERE user_id = $1")
                    .bind(DEV_USER_ID)
                    .execute(&mut **tx)
                    .await?;
            }
            "tags" => {
                sqlx::query("DELETE FROM tag WHERE user_id = $1")
                    .bind(DEV_USER_ID)
                    .execute(&mut **tx)
                    .await?;
                sqlx::query("DELETE FROM tag_type WHERE user_id = $1")
                    .bind(DEV_USER_ID)
                    .execute(&mut **tx)
                    .await?;
            }
            "codebases" => {
                sqlx::query("DELETE FROM space WHERE user_id = $1")
                    .bind(DEV_USER_ID)
                    .execute(&mut **tx)
                    .await?;
            }
            // The remaining legacy modules have no reset hook.
            "core" | "connections" | "file-links" | "self-documenting" => {}
            _ => anyhow::bail!("Unknown seed module reset: {module}"),
        }
    }
    Ok(())
}
