use sqlx::{Connection, PgConnection};

pub(crate) static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

const REQUIRED_PUBLIC_RELATIONS: [&str; 5] = [
    "agent_run",
    "plan_task_claim",
    "coordination_entry",
    "projection_outbox",
    "account",
];

/// Tables created by migration 0001. A pre-SQLx database is only eligible
/// for Drizzle baseline adoption when every one is present; checking the
/// complete signature prevents a partial or stale schema from being marked
/// as migrated and failing later in less obvious ways.
const BASELINE_TABLES: [&str; 58] = [
    "activity_log",
    "behavior_cell",
    "behavior_cell_requirement",
    "behavior_dimension",
    "behavior_matrix",
    "behavior_rule",
    "chunk",
    "chunk_applies_to",
    "chunk_comment",
    "chunk_connection",
    "chunk_feature_delta",
    "chunk_file_ref",
    "chunk_proposal",
    "chunk_space",
    "chunk_staleness",
    "chunk_tag",
    "chunk_template",
    "chunk_type",
    "chunk_version",
    "codebase_settings",
    "collection",
    "connection_relation",
    "context_snapshot",
    "document",
    "feature",
    "feature_space",
    "instance_settings",
    "learning_path",
    "notification",
    "plan",
    "plan_analyze_item",
    "plan_external_link",
    "plan_requirement",
    "plan_task",
    "plan_task_chunk",
    "plan_task_dependency",
    "plan_task_external_link",
    "requirement",
    "requirement_chunk",
    "requirement_dependency",
    "saved_graph",
    "saved_query",
    "scope_key",
    "session",
    "space",
    "space_code_metadata",
    "space_kind",
    "staleness_scan",
    "tag",
    "tag_type",
    "use_case",
    "user",
    "user_active_feature",
    "user_favorite",
    "user_settings",
    "vocabulary_entry",
    "workspace",
    "workspace_space",
];

/// The checked-in Drizzle migration set predates the behavior-matrix base
/// tables even though the final TypeScript schema includes them. Repairing
/// exactly this all-five-missing signature is deterministic; any subset is
/// treated as an unknown partial migration and rejected below.
const DRIZZLE_BEHAVIOR_GAP: [&str; 5] = [
    "behavior_cell",
    "behavior_cell_requirement",
    "behavior_dimension",
    "behavior_matrix",
    "behavior_rule",
];

const CREATE_DRIZZLE_BEHAVIOR_GAP: &str = r#"
CREATE TABLE behavior_matrix (
    id text PRIMARY KEY,
    name text NOT NULL,
    layer text NOT NULL,
    description text,
    space_id text,
    user_id text NOT NULL,
    created_at timestamp without time zone DEFAULT now() NOT NULL,
    updated_at timestamp without time zone DEFAULT now() NOT NULL,
    CONSTRAINT behavior_matrix_space_id_space_id_fk
        FOREIGN KEY (space_id) REFERENCES space(id) ON DELETE SET NULL,
    CONSTRAINT behavior_matrix_user_id_user_id_fk
        FOREIGN KEY (user_id) REFERENCES "user"(id) ON DELETE CASCADE
);
CREATE INDEX behavior_matrix_layer_idx ON behavior_matrix(layer);
CREATE INDEX "behavior_matrix_userId_idx" ON behavior_matrix(user_id);

CREATE TABLE behavior_dimension (
    id text PRIMARY KEY,
    matrix_id text NOT NULL,
    name text NOT NULL,
    "order" integer DEFAULT 0 NOT NULL,
    created_at timestamp without time zone DEFAULT now() NOT NULL,
    CONSTRAINT behavior_dimension_matrix_name UNIQUE (matrix_id, name),
    CONSTRAINT behavior_dimension_matrix_id_behavior_matrix_id_fk
        FOREIGN KEY (matrix_id) REFERENCES behavior_matrix(id) ON DELETE CASCADE
);
CREATE INDEX "behavior_dimension_matrixId_idx" ON behavior_dimension(matrix_id);

CREATE TABLE behavior_rule (
    id text PRIMARY KEY,
    matrix_id text NOT NULL,
    title text NOT NULL,
    description text,
    category text,
    "order" integer DEFAULT 0 NOT NULL,
    created_at timestamp without time zone DEFAULT now() NOT NULL,
    updated_at timestamp without time zone DEFAULT now() NOT NULL,
    CONSTRAINT behavior_rule_matrix_id_behavior_matrix_id_fk
        FOREIGN KEY (matrix_id) REFERENCES behavior_matrix(id) ON DELETE CASCADE
);
CREATE INDEX "behavior_rule_matrixId_idx" ON behavior_rule(matrix_id);

CREATE TABLE behavior_cell (
    id text PRIMARY KEY,
    rule_id text NOT NULL,
    dimension_id text NOT NULL,
    created_at timestamp without time zone DEFAULT now() NOT NULL,
    CONSTRAINT behavior_cell_rule_dimension UNIQUE (rule_id, dimension_id),
    CONSTRAINT behavior_cell_rule_id_behavior_rule_id_fk
        FOREIGN KEY (rule_id) REFERENCES behavior_rule(id) ON DELETE CASCADE,
    CONSTRAINT behavior_cell_dimension_id_behavior_dimension_id_fk
        FOREIGN KEY (dimension_id) REFERENCES behavior_dimension(id) ON DELETE CASCADE
);
CREATE INDEX "behavior_cell_ruleId_idx" ON behavior_cell(rule_id);
CREATE INDEX "behavior_cell_dimensionId_idx" ON behavior_cell(dimension_id);

CREATE TABLE behavior_cell_requirement (
    cell_id text NOT NULL,
    requirement_id text NOT NULL,
    CONSTRAINT behavior_cell_requirement_cell_id_requirement_id_pk
        PRIMARY KEY (cell_id, requirement_id),
    CONSTRAINT behavior_cell_requirement_cell_id_behavior_cell_id_fk
        FOREIGN KEY (cell_id) REFERENCES behavior_cell(id) ON DELETE CASCADE,
    CONSTRAINT behavior_cell_requirement_requirement_id_requirement_id_fk
        FOREIGN KEY (requirement_id) REFERENCES requirement(id) ON DELETE CASCADE
);
"#;

fn configuration_error(message: impl Into<String>) -> sqlx::Error {
    sqlx::Error::Configuration(Box::new(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        message.into(),
    )))
}

/// Verifies the schema-placement contract expected by every application
/// query. This deliberately runs after all migrations so a broken migration
/// fails startup at the migration seam rather than much later in a feature.
pub(crate) async fn validate_public_schema(conn: &mut PgConnection) -> Result<(), sqlx::Error> {
    let required: Vec<String> = REQUIRED_PUBLIC_RELATIONS
        .iter()
        .map(|name| (*name).to_owned())
        .collect();
    let missing: Vec<String> = sqlx::query_scalar(
        "SELECT required.name \
         FROM unnest($1::text[]) AS required(name) \
         LEFT JOIN information_schema.tables actual \
           ON actual.table_schema = 'public' AND actual.table_name = required.name \
         WHERE actual.table_name IS NULL \
         ORDER BY required.name",
    )
    .bind(&required)
    .fetch_all(&mut *conn)
    .await?;
    if missing.is_empty() {
        return Ok(());
    }

    let misplaced: Vec<String> = sqlx::query_scalar(
        "SELECT format('%s.%s', actual.table_schema, actual.table_name) \
         FROM information_schema.tables actual \
         WHERE actual.table_name = ANY($1::text[]) \
           AND actual.table_schema <> 'public' \
         ORDER BY actual.table_schema, actual.table_name",
    )
    .bind(&missing)
    .fetch_all(&mut *conn)
    .await?;
    Err(configuration_error(format!(
        "database migration schema contract failed; missing from public: {}; \
         same-named relations outside public: {}",
        missing.join(", "),
        if misplaced.is_empty() {
            "none".to_owned()
        } else {
            misplaced.join(", ")
        }
    )))
}

/// Adopts migration 0001 for a database created by the retired Drizzle
/// runtime. Only the baseline is adopted: migrations 0002+ remain pending
/// and are applied by SQLx immediately afterward.
///
/// The Drizzle marker alone is insufficient because its checked-in journal
/// did not track every historical schema change. The complete table
/// signature is therefore required before writing SQLx bookkeeping. An
/// existing application schema without that proof fails closed with a
/// useful error rather than running 0001 into `relation already exists` or,
/// worse, pretending a partial schema is current.
pub(crate) async fn adopt_legacy_drizzle_baseline(
    conn: &mut PgConnection,
) -> Result<(), sqlx::Error> {
    let mut tx = conn.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext('fubbik:drizzle-baseline-adoption'))")
        .execute(&mut *tx)
        .await?;

    let sqlx_history_exists: bool =
        sqlx::query_scalar("SELECT to_regclass('public._sqlx_migrations') IS NOT NULL")
            .fetch_one(&mut *tx)
            .await?;
    if sqlx_history_exists {
        tx.commit().await?;
        return Ok(());
    }

    let drizzle_history_exists: bool =
        sqlx::query_scalar("SELECT to_regclass('drizzle.__drizzle_migrations') IS NOT NULL")
            .fetch_one(&mut *tx)
            .await?;
    let any_application_schema: bool = sqlx::query_scalar(
        "SELECT to_regclass('public.chunk') IS NOT NULL \
         OR to_regclass('public.\"user\"') IS NOT NULL",
    )
    .fetch_one(&mut *tx)
    .await?;

    if !drizzle_history_exists {
        if any_application_schema {
            return Err(configuration_error(
                "database contains Fubbik tables but has neither SQLx nor Drizzle migration \
                 history; refusing automatic baseline adoption. Back up the database and \
                 migrate it from a recognized Drizzle deployment or start with an empty \
                 database",
            ));
        }
        tx.commit().await?;
        return Ok(());
    }

    let table_names: Vec<String> = BASELINE_TABLES
        .iter()
        .map(|name| (*name).to_owned())
        .collect();
    let mut missing_tables: Vec<String> = sqlx::query_scalar(
        "SELECT required.name \
         FROM unnest($1::text[]) AS required(name) \
         LEFT JOIN information_schema.tables actual \
           ON actual.table_schema = 'public' AND actual.table_name = required.name \
         WHERE actual.table_name IS NULL \
         ORDER BY required.name",
    )
    .bind(&table_names)
    .fetch_all(&mut *tx)
    .await?;
    if missing_tables == DRIZZLE_BEHAVIOR_GAP {
        sqlx::raw_sql(CREATE_DRIZZLE_BEHAVIOR_GAP)
            .execute(&mut *tx)
            .await?;
        missing_tables.clear();
        tracing::info!(
            "repaired behavior-matrix tables omitted by the legacy Drizzle migration set"
        );
    }
    if !missing_tables.is_empty() {
        return Err(configuration_error(format!(
            "legacy Drizzle database is not compatible with the Rust baseline; missing \
             tables: {}. No SQLx migration history was written. Apply the outstanding \
             legacy schema migrations first or restore into a fresh database",
            missing_tables.join(", ")
        )));
    }

    let baseline = MIGRATOR
        .iter()
        .find(|migration| migration.version == 1)
        .expect("embedded migration set must contain baseline version 1");
    sqlx::raw_sql(
        "CREATE TABLE _sqlx_migrations (\
             version BIGINT PRIMARY KEY,\
             description TEXT NOT NULL,\
             installed_on TIMESTAMPTZ NOT NULL DEFAULT now(),\
             success BOOLEAN NOT NULL,\
             checksum BYTEA NOT NULL,\
             execution_time BIGINT NOT NULL\
         )",
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "INSERT INTO _sqlx_migrations \
         (version, description, success, checksum, execution_time) \
         VALUES ($1, $2, true, $3, 0)",
    )
    .bind(baseline.version)
    .bind(baseline.description.as_ref())
    .bind(baseline.checksum.as_ref())
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    tracing::info!(
        migration_version = baseline.version,
        "adopted verified legacy Drizzle schema as SQLx migration baseline"
    );
    Ok(())
}
