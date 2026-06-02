-- Phase A of domain-agnostic-database: codebase → space rename.
-- Idempotent (uses IF NOT EXISTS / IF EXISTS where possible).

-- 1. space_kind lookup + seeds
CREATE TABLE IF NOT EXISTS space_kind (
    id text PRIMARY KEY,
    label text NOT NULL,
    description text,
    icon text,
    display_order integer NOT NULL DEFAULT 100,
    built_in boolean NOT NULL DEFAULT false,
    created_at timestamp NOT NULL DEFAULT now(),
    updated_at timestamp NOT NULL DEFAULT now()
);

INSERT INTO space_kind (id, label, description, icon, display_order, built_in) VALUES
    ('code',     'Code',     'A code repository or codebase',                 'Code',         10, true),
    ('wiki',     'Wiki',     'A general-purpose knowledge wiki',              'BookOpen',     20, true),
    ('notes',    'Notes',    'Personal notes, journals, or scratch space',    'Notebook',     30, true),
    ('research', 'Research', 'Research artefacts, references, and findings', 'FlaskConical', 40, true)
ON CONFLICT (id) DO NOTHING;

-- 2. space table
CREATE TABLE IF NOT EXISTS space (
    id text PRIMARY KEY,
    name text NOT NULL,
    kind text NOT NULL REFERENCES space_kind(id) ON DELETE RESTRICT,
    description text,
    user_id text NOT NULL REFERENCES "user"(id) ON DELETE CASCADE,
    created_at timestamp NOT NULL DEFAULT now(),
    updated_at timestamp NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX IF NOT EXISTS space_user_name_idx ON space(user_id, name);
CREATE INDEX IF NOT EXISTS space_userId_idx ON space(user_id);
CREATE INDEX IF NOT EXISTS space_kind_idx ON space(kind);

-- 3. space_code_metadata side-table
CREATE TABLE IF NOT EXISTS space_code_metadata (
    space_id text PRIMARY KEY REFERENCES space(id) ON DELETE CASCADE,
    user_id text NOT NULL,
    remote_url text,
    local_paths jsonb NOT NULL DEFAULT '[]'::jsonb
);
CREATE UNIQUE INDEX IF NOT EXISTS space_code_user_remote_idx
    ON space_code_metadata(user_id, remote_url) WHERE remote_url IS NOT NULL;

BEGIN;

-- 4. Copy codebase → space + space_code_metadata (guarded: codebase may already be dropped)
DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM information_schema.tables
        WHERE table_schema = 'public' AND table_name = 'codebase'
    ) THEN
        INSERT INTO space (id, name, kind, description, user_id, created_at, updated_at)
        SELECT id, name, 'code', NULL, user_id, created_at, updated_at
        FROM codebase
        ON CONFLICT (id) DO NOTHING;

        INSERT INTO space_code_metadata (space_id, user_id, remote_url, local_paths)
        SELECT id, user_id, remote_url, COALESCE(local_paths, '[]'::jsonb)
        FROM codebase
        ON CONFLICT (space_id) DO NOTHING;

        RAISE NOTICE 'Copied % codebase row(s) to space', (SELECT count(*) FROM codebase);
    ELSE
        RAISE NOTICE 'codebase table not present — skipping copy (already migrated)';
    END IF;
END $$;

-- 5. chunk_space (renamed from chunk_codebase)
CREATE TABLE IF NOT EXISTS chunk_space (
    chunk_id text NOT NULL REFERENCES chunk(id) ON DELETE CASCADE,
    space_id text NOT NULL REFERENCES space(id) ON DELETE CASCADE,
    PRIMARY KEY (chunk_id, space_id)
);
CREATE INDEX IF NOT EXISTS chunk_space_chunkId_idx ON chunk_space(chunk_id);
CREATE INDEX IF NOT EXISTS chunk_space_spaceId_idx ON chunk_space(space_id);

DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM information_schema.tables
        WHERE table_schema = 'public' AND table_name = 'chunk_codebase'
    ) THEN
        INSERT INTO chunk_space (chunk_id, space_id)
        SELECT chunk_id, codebase_id FROM chunk_codebase
        ON CONFLICT DO NOTHING;
        RAISE NOTICE 'Copied % chunk_codebase row(s) to chunk_space', (SELECT count(*) FROM chunk_codebase);
    ELSE
        RAISE NOTICE 'chunk_codebase table not present — skipping copy (already migrated)';
    END IF;
END $$;

-- 6. workspace_space (renamed from workspace_codebase)
CREATE TABLE IF NOT EXISTS workspace_space (
    workspace_id text NOT NULL REFERENCES workspace(id) ON DELETE CASCADE,
    space_id text NOT NULL REFERENCES space(id) ON DELETE CASCADE,
    PRIMARY KEY (workspace_id, space_id)
);
CREATE INDEX IF NOT EXISTS workspace_space_workspaceId_idx ON workspace_space(workspace_id);
CREATE INDEX IF NOT EXISTS workspace_space_spaceId_idx ON workspace_space(space_id);

DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM information_schema.tables
        WHERE table_schema = 'public' AND table_name = 'workspace_codebase'
    ) THEN
        INSERT INTO workspace_space (workspace_id, space_id)
        SELECT workspace_id, codebase_id FROM workspace_codebase
        ON CONFLICT DO NOTHING;
        RAISE NOTICE 'Copied % workspace_codebase row(s) to workspace_space', (SELECT count(*) FROM workspace_codebase);
    ELSE
        RAISE NOTICE 'workspace_codebase table not present — skipping copy (already migrated)';
    END IF;
END $$;

-- 7. Rename codebase_id → space_id on every table that has it.
--
-- onDelete behaviour per schema cross-check:
--   CASCADE:   chunk_type, connection_relation, codebase_settings,
--              feature_codebase, staleness_scan, vocabulary_entry
--   SET NULL:  activity_log, behavior_matrix, collection, document,
--              plan, requirement, saved_graph, saved_query, use_case
DO $$
DECLARE
    tbl text;
    -- All tables that have a codebase_id column referencing codebase.id directly
    -- (chunk_codebase and workspace_codebase are handled via the new join tables above)
    tables text[] := ARRAY[
        'activity_log',
        'behavior_matrix',
        'chunk_type',
        'codebase_settings',
        'collection',
        'connection_relation',
        'document',
        'feature_codebase',
        'plan',
        'requirement',
        'saved_graph',
        'saved_query',
        'staleness_scan',
        'use_case',
        'vocabulary_entry'
    ];
    cascade_tables text[] := ARRAY[
        'chunk_type',
        'codebase_settings',
        'connection_relation',
        'feature_codebase',
        'staleness_scan',
        'vocabulary_entry'
    ];
BEGIN
    FOREACH tbl IN ARRAY tables LOOP
        IF EXISTS (
            SELECT 1 FROM information_schema.columns
            WHERE table_schema = 'public'
              AND table_name = tbl
              AND column_name = 'codebase_id'
        ) AND NOT EXISTS (
            SELECT 1 FROM information_schema.columns
            WHERE table_schema = 'public'
              AND table_name = tbl
              AND column_name = 'space_id'
        ) THEN
            -- Rename the column
            EXECUTE format('ALTER TABLE %I RENAME COLUMN codebase_id TO space_id', tbl);

            -- Drop the old FK constraint (Drizzle names it <table>_codebase_id_codebase_id_fk)
            EXECUTE format(
                'ALTER TABLE %I DROP CONSTRAINT IF EXISTS %I',
                tbl,
                tbl || '_codebase_id_codebase_id_fk'
            );

            -- Add new FK pointing at space, preserving the original onDelete behaviour
            EXECUTE format(
                'ALTER TABLE %I ADD CONSTRAINT %I FOREIGN KEY (space_id) REFERENCES space(id) ON DELETE %s',
                tbl,
                tbl || '_space_id_space_id_fk',
                CASE WHEN tbl = ANY(cascade_tables) THEN 'CASCADE' ELSE 'SET NULL' END
            );

            RAISE NOTICE 'Renamed codebase_id → space_id on table %', tbl;
        ELSE
            RAISE NOTICE 'Skipping table % (already migrated or codebase_id not present)', tbl;
        END IF;
    END LOOP;
END $$;

-- 7b. Rename feature_codebase → feature_space (separate from section 7's column rename loop
-- because this is a TABLE rename, not a column rename).
DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM information_schema.tables
        WHERE table_schema = 'public' AND table_name = 'feature_codebase'
    ) AND NOT EXISTS (
        SELECT 1 FROM information_schema.tables
        WHERE table_schema = 'public' AND table_name = 'feature_space'
    ) THEN
        ALTER TABLE feature_codebase RENAME TO feature_space;
        ALTER TABLE feature_space RENAME CONSTRAINT feature_codebase_feature_id_codebase_id_pk TO feature_space_feature_id_space_id_pk;
        ALTER TABLE feature_space RENAME CONSTRAINT feature_codebase_feature_id_feature_id_fk TO feature_space_feature_id_feature_id_fk;
        ALTER TABLE feature_space RENAME CONSTRAINT feature_codebase_space_id_space_id_fk TO feature_space_space_id_space_id_fk;
    END IF;
END $$;

-- 8. Drop old join tables and codebase itself (IF EXISTS for idempotency)
DROP TABLE IF EXISTS chunk_codebase;
DROP TABLE IF EXISTS workspace_codebase;
DROP TABLE IF EXISTS codebase;

COMMIT;
