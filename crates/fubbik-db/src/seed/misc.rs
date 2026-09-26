use anyhow::Result;
use sqlx::{Postgres, Transaction};

use super::spaces::DEV_USER_ID;

async fn fubbik_space(tx: &mut Transaction<'_, Postgres>) -> Result<String> {
    Ok(
        sqlx::query_scalar("SELECT id FROM space WHERE user_id = $1 AND name = 'fubbik'")
            .bind(DEV_USER_ID)
            .fetch_one(&mut **tx)
            .await?,
    )
}

pub async fn seed_documents(tx: &mut Transaction<'_, Postgres>) -> Result<usize> {
    let space_id = fubbik_space(tx).await?;
    let fixtures = [
        (
            "docs-getting-started",
            "Getting Started",
            "docs/guide/getting-started.md",
            "Install, seed, run dev — the 60-second path to a live fubbik",
        ),
        (
            "docs-architecture",
            "Architecture",
            "docs/guide/architecture.md",
            "Monorepo layout, request flow, error handling",
        ),
        (
            "docs-chunks",
            "Chunks",
            "docs/guide/chunks.md",
            "How chunks work, what types exist, how to capture one",
        ),
    ];
    for (name, title, path, description) in fixtures {
        let existing: Option<String> =
            sqlx::query_scalar("SELECT id FROM document WHERE user_id = $1 AND source_path = $2")
                .bind(DEV_USER_ID)
                .bind(path)
                .fetch_optional(&mut **tx)
                .await?;
        let id = existing.unwrap_or_else(|| format!("seed-{name}"));
        sqlx::query(
            "INSERT INTO document (id, title, source_path, content_hash, description, user_id, space_id) \
             VALUES ($1, $2, $3, 'seed-placeholder', $4, $5, $6) ON CONFLICT (id) DO UPDATE SET \
             title = EXCLUDED.title, description = EXCLUDED.description, updated_at = now()",
        )
        .bind(id).bind(title).bind(path).bind(description).bind(DEV_USER_ID).bind(&space_id)
        .execute(&mut **tx).await?;
    }
    Ok(fixtures.len())
}

pub async fn seed_vocabulary(tx: &mut Transaction<'_, Postgres>) -> Result<usize> {
    let space_id = fubbik_space(tx).await?;
    let fixtures: &[(&str, &str, Option<&[&str]>)] = &[
        ("chunk", "noun", None),
        ("plan", "noun", None),
        ("codebase", "noun", None),
        ("tag", "noun", None),
        ("capture", "verb", Some(&["chunk", "note"])),
        ("link", "verb", Some(&["chunk", "chunk"])),
        ("seed", "verb", None),
        ("apply", "verb", Some(&["filter", "plan"])),
        ("must", "modal", None),
        ("should", "modal", None),
    ];
    for (word, category, expects) in fixtures {
        let existing: Option<String> = sqlx::query_scalar(
            "SELECT id FROM vocabulary_entry WHERE user_id = $1 AND space_id = $2 AND word = $3 AND category = $4",
        )
        .bind(DEV_USER_ID).bind(&space_id).bind(word).bind(category)
        .fetch_optional(&mut **tx).await?;
        let id = existing.unwrap_or_else(|| format!("seed-vocab-{word}"));
        sqlx::query(
            "INSERT INTO vocabulary_entry (id, word, category, expects, space_id, user_id) \
             VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT (id) DO UPDATE SET \
             expects = EXCLUDED.expects, updated_at = now()",
        )
        .bind(id)
        .bind(word)
        .bind(category)
        .bind(expects.map(|v| serde_json::json!(v)))
        .bind(&space_id)
        .bind(DEV_USER_ID)
        .execute(&mut **tx)
        .await?;
    }
    Ok(fixtures.len())
}

pub async fn seed_workspaces(tx: &mut Transaction<'_, Postgres>) -> Result<usize> {
    let existing: Option<String> = sqlx::query_scalar(
        "SELECT id FROM workspace WHERE user_id = $1 AND name = 'fubbik-platform'",
    )
    .bind(DEV_USER_ID)
    .fetch_optional(&mut **tx)
    .await?;
    let id = existing.unwrap_or_else(|| "seed-workspace-platform".to_string());
    sqlx::query(
        "INSERT INTO workspace (id, name, description, user_id) VALUES ($1, 'fubbik-platform', \
         'Main fubbik product — code + docs grouped together', $2) \
         ON CONFLICT (id) DO UPDATE SET description = EXCLUDED.description, updated_at = now()",
    )
    .bind(&id)
    .bind(DEV_USER_ID)
    .execute(&mut **tx)
    .await?;
    sqlx::query(
        "INSERT INTO workspace_space (workspace_id, space_id) \
         SELECT $1, id FROM space WHERE user_id = $2 AND name IN ('fubbik', 'fubbik-docs') \
         ON CONFLICT DO NOTHING",
    )
    .bind(&id)
    .bind(DEV_USER_ID)
    .execute(&mut **tx)
    .await?;
    Ok(1)
}

pub async fn seed_collections(tx: &mut Transaction<'_, Postgres>) -> Result<usize> {
    let space_id = fubbik_space(tx).await?;
    let fixtures = [
        (
            "Conventions",
            "Pinned conventions — always include in AI context",
            serde_json::json!({"type": "convention"}),
        ),
        (
            "Self-documenting",
            "Chunks that describe fubbik itself",
            serde_json::json!({"tags": "self-documenting"}),
        ),
    ];
    for (name, description, filter) in &fixtures {
        let existing: Option<String> =
            sqlx::query_scalar("SELECT id FROM collection WHERE user_id = $1 AND name = $2")
                .bind(DEV_USER_ID)
                .bind(name)
                .fetch_optional(&mut **tx)
                .await?;
        let id = existing.unwrap_or_else(|| format!("seed-collection-{}", name.to_lowercase()));
        sqlx::query(
            "INSERT INTO collection (id, name, description, filter, user_id, space_id) \
             VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT (id) DO UPDATE SET \
             description = EXCLUDED.description, filter = EXCLUDED.filter, updated_at = now()",
        )
        .bind(id)
        .bind(name)
        .bind(description)
        .bind(filter)
        .bind(DEV_USER_ID)
        .bind(&space_id)
        .execute(&mut **tx)
        .await?;
    }
    Ok(fixtures.len())
}
