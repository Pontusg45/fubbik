use anyhow::Result;
use serde::Serialize;
use sqlx::{Postgres, Transaction};
use std::collections::BTreeMap;

use super::spaces::DEV_USER_ID;

struct Fixture {
    name: &'static str,
    title: &'static str,
    kind: &'static str,
    content: &'static str,
    pattern: &'static str,
    tags: &'static [&'static str],
}

const FIXTURES: &[Fixture] = &[
    Fixture {
        name: "seed-architecture",
        title: "Seed system: architecture overview",
        kind: "document",
        content: "The Rust seed lives in crates/fubbik-db/src/seed. The planner closes module dependencies, then the orchestrator runs selected modules in one PostgreSQL transaction. The fubbik seed CLI exposes scenario, only, skip, and reset options. A failure rolls back the entire run.",
        pattern: "crates/fubbik-db/src/seed/**",
        tags: &[
            "seed-system",
            "self-documenting",
            "architecture",
            "onboarding",
        ],
    },
    Fixture {
        name: "seed-fixtures-dsl",
        title: "Seed system: fixture model",
        kind: "reference",
        content: "Fixtures identify chunks and relationships by semantic names. A module records the database ID for each name, and later modules resolve those names before inserting foreign keys. A missing name is an error. Catalog data is checked into JSON files alongside the Rust modules.",
        pattern: "crates/fubbik-db/src/seed/*-catalog.json",
        tags: &["seed-system", "self-documenting", "reference"],
    },
    Fixture {
        name: "seed-scenarios",
        title: "Seed system: scenarios",
        kind: "reference",
        content: "Minimal seeds the built-in catalog and seed documentation with its codebase and tag dependencies. Demo seeds all registered fixture modules. Extended currently uses the same modules and can add richer fixtures later. The planner accepts --only and --skip, includes transitive dependencies, and rejects conflicts or cycles.",
        pattern: "crates/fubbik-db/src/seed/planner.rs",
        tags: &["seed-system", "self-documenting", "reference"],
    },
    Fixture {
        name: "seed-adding-a-module",
        title: "Seed system: how to add a new module",
        kind: "guide",
        content: "Add the module to the planner registry, implement its Rust seed function inside crates/fubbik-db/src/seed, dispatch it from seed(), and add database-backed parity and idempotence tests. Keep seed-owned rows distinguishable from user data and resolve cross-module references by name.",
        pattern: "crates/fubbik-db/src/seed/*.rs",
        tags: &["seed-system", "self-documenting", "onboarding"],
    },
    Fixture {
        name: "seed-factories",
        title: "Seed system: fixture helpers",
        kind: "reference",
        content: "Each seed module inserts its own domain rows using stable names and IDs. Shared fixture helpers are planned for the remaining module ports; preserve ownership, type, provenance, and relation values when introducing them.",
        pattern: "crates/fubbik-db/src/seed/mod.rs",
        tags: &["seed-system", "self-documenting", "reference"],
    },
    Fixture {
        name: "seed-verification",
        title: "Seed system: verification",
        kind: "reference",
        content: "After seeding, compare the structured report and database row counts with the legacy fixtures. Verify named relationships, foreign keys, repeat-run behavior, and rollback. The entire run commits only when every module succeeds.",
        pattern: "crates/fubbik-db/tests/seed.rs",
        tags: &["seed-system", "self-documenting", "reference"],
    },
];

const LINKS: &[(&str, &str, &str)] = &[
    ("seed-architecture", "seed-fixtures-dsl", "part_of"),
    ("seed-architecture", "seed-scenarios", "part_of"),
    ("seed-architecture", "seed-adding-a-module", "part_of"),
    ("seed-architecture", "seed-factories", "part_of"),
    ("seed-architecture", "seed-verification", "part_of"),
    ("seed-fixtures-dsl", "seed-factories", "depends_on"),
    ("seed-adding-a-module", "seed-architecture", "references"),
];

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize)]
pub struct SelfDocCounts {
    pub chunks: usize,
    pub connections: usize,
    pub applies_to: usize,
}

pub async fn seed_self_documenting(tx: &mut Transaction<'_, Postgres>) -> Result<SelfDocCounts> {
    let space_id: Option<String> =
        sqlx::query_scalar("SELECT id FROM space WHERE user_id = $1 AND name = 'fubbik'")
            .bind(DEV_USER_ID)
            .fetch_optional(&mut **tx)
            .await?;
    let mut ids = BTreeMap::new();
    for fixture in FIXTURES {
        let existing: Option<String> =
            sqlx::query_scalar("SELECT id FROM chunk WHERE user_id = $1 AND title = $2")
                .bind(DEV_USER_ID)
                .bind(fixture.title)
                .fetch_optional(&mut **tx)
                .await?;
        let id = existing.unwrap_or_else(|| format!("seed-selfdoc-{}", fixture.name));
        sqlx::query(
            "INSERT INTO chunk (id, title, type, content, user_id) VALUES ($1, $2, $3, $4, $5) \
             ON CONFLICT (id) DO UPDATE SET content = EXCLUDED.content, updated_at = now()",
        )
        .bind(&id)
        .bind(fixture.title)
        .bind(fixture.kind)
        .bind(fixture.content)
        .bind(DEV_USER_ID)
        .execute(&mut **tx)
        .await?;
        if let Some(space_id) = &space_id {
            sqlx::query("INSERT INTO chunk_space (chunk_id, space_id) VALUES ($1, $2) ON CONFLICT DO NOTHING")
                .bind(&id).bind(space_id).execute(&mut **tx).await?;
        }
        for tag in fixture.tags {
            sqlx::query(
                "INSERT INTO chunk_tag (chunk_id, tag_id) \
                 SELECT $1, id FROM tag WHERE user_id = $2 AND name = $3 \
                 ON CONFLICT DO NOTHING",
            )
            .bind(&id)
            .bind(DEV_USER_ID)
            .bind(tag)
            .execute(&mut **tx)
            .await?;
        }
        sqlx::query("DELETE FROM chunk_applies_to WHERE chunk_id = $1 AND pattern = $2")
            .bind(&id)
            .bind(fixture.pattern)
            .execute(&mut **tx)
            .await?;
        sqlx::query("INSERT INTO chunk_applies_to (id, chunk_id, pattern) VALUES ($1, $2, $3)")
            .bind(format!("{id}-applies"))
            .bind(&id)
            .bind(fixture.pattern)
            .execute(&mut **tx)
            .await?;
        ids.insert(fixture.name, id);
    }
    for (from, to, relation) in LINKS {
        let source_id = &ids[from];
        let target_id = &ids[to];
        sqlx::query(
            "INSERT INTO chunk_connection (id, source_id, target_id, relation) \
             VALUES ($1, $2, $3, $4) ON CONFLICT (source_id, target_id, relation) DO NOTHING",
        )
        .bind(format!("seed-selfdoc-link-{from}-{to}"))
        .bind(source_id)
        .bind(target_id)
        .bind(relation)
        .execute(&mut **tx)
        .await?;
    }
    Ok(SelfDocCounts {
        chunks: FIXTURES.len(),
        connections: LINKS.len(),
        applies_to: FIXTURES.len(),
    })
}
