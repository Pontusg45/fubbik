-- Brings the `behavior_*` schema up to what Node's Drizzle definitions
-- (`packages/db/src/schema/behavior-matrix.ts`) actually declare.
--
-- WHY THIS EXISTS
--
-- `0001_init.sql` is a `pg_dump` of the Node database taken at the start of
-- the port, and it predates part of the behavioral-matrix feature. Three of
-- Node's eight `behavior_*` tables were never in that dump, and
-- `behavior_rule` was missing four columns. Nothing noticed, because no Rust
-- code referenced any of them until the matrices domain was ported.
--
-- Found by diffing every `pgTable(...)` in Node's schema against every
-- `CREATE TABLE` in these migrations, column by column, rather than by
-- discovering each gap as its query failed to compile. The full diff over
-- all 65 Node tables came back with exactly these three tables and these
-- four columns — every other shared table is column-complete, which is the
-- part worth knowing.
--
-- `graph_event`, `usage_event`, `account` and `verification` are also absent
-- from these migrations and are deliberately left that way: the first two
-- have no reader in `packages/api/src` at all, and the last two belong to
-- better-auth, which Rust does not use (it has its own `user`/`session`
-- tables and its own argon2 password path).
--
-- IF NOT EXISTS is load-bearing, not defensive habit. Node's Drizzle has
-- already created all of this in every database the Node stack has touched,
-- including the one Rust is meant to take over at the end of the port. A
-- bare CREATE TABLE would abort this migration there and leave the
-- `_sqlx_migrations` row unapplied, so the two stacks could never share a
-- database. The same reasoning applies to each ADD COLUMN.

-- ---------------------------------------------------------------------------
-- behavior_rule: decision-context columns
-- ---------------------------------------------------------------------------
--
-- `rationale`/`alternatives`/`consequences` mirror the same three fields on
-- `chunk` (the "why" behind a decision); `counterexample` is this table's
-- own idea — an explicit statement of what violating the behaviour looks
-- like. All four are nullable free text, matching Node exactly. Note
-- `alternatives` is `text` here, NOT the `jsonb` array `chunk.alternatives`
-- uses — the two fields share a name and not a type.

ALTER TABLE public.behavior_rule ADD COLUMN IF NOT EXISTS rationale text;
ALTER TABLE public.behavior_rule ADD COLUMN IF NOT EXISTS alternatives text;
ALTER TABLE public.behavior_rule ADD COLUMN IF NOT EXISTS consequences text;
ALTER TABLE public.behavior_rule ADD COLUMN IF NOT EXISTS counterexample text;

-- ---------------------------------------------------------------------------
-- behavior_rule_version: append-only rule history
-- ---------------------------------------------------------------------------
--
-- `snapshot` is plain `jsonb` rather than a set of mirrored columns, so the
-- shape of a recorded version is whatever the rule looked like at the time
-- and does not have to be migrated when `behavior_rule` grows a field.
--
-- `changed_by` is ON DELETE SET NULL, not CASCADE: deleting a user must not
-- erase the history of rules they edited. That asymmetry against `rule_id`'s
-- CASCADE is deliberate in Node and reproduced here.

CREATE TABLE IF NOT EXISTS public.behavior_rule_version (
    id text NOT NULL,
    rule_id text NOT NULL,
    snapshot jsonb NOT NULL,
    changed_by text,
    created_at timestamp without time zone DEFAULT now() NOT NULL
);

DO $$ BEGIN
    ALTER TABLE ONLY public.behavior_rule_version
        ADD CONSTRAINT behavior_rule_version_pkey PRIMARY KEY (id);
EXCEPTION WHEN duplicate_table OR invalid_table_definition THEN NULL;
END $$;

DO $$ BEGIN
    ALTER TABLE ONLY public.behavior_rule_version
        ADD CONSTRAINT behavior_rule_version_rule_id_fkey
        FOREIGN KEY (rule_id) REFERENCES public.behavior_rule(id) ON DELETE CASCADE;
EXCEPTION WHEN duplicate_object THEN NULL;
END $$;

DO $$ BEGIN
    ALTER TABLE ONLY public.behavior_rule_version
        ADD CONSTRAINT behavior_rule_version_changed_by_fkey
        FOREIGN KEY (changed_by) REFERENCES public."user"(id) ON DELETE SET NULL;
EXCEPTION WHEN duplicate_object THEN NULL;
END $$;

CREATE INDEX IF NOT EXISTS "behavior_rule_version_ruleId_idx"
    ON public.behavior_rule_version USING btree (rule_id);

-- ---------------------------------------------------------------------------
-- behavior_cell_code: links a cell to the code that implements or verifies it
-- ---------------------------------------------------------------------------
--
-- `kind` is `file | symbol | test` and `ref` is a path, a `path::symbol`, or
-- a test identifier. Both are plain `text` with no CHECK — Node constrains
-- `kind` only on the write route's schema, so older rows may hold anything
-- and the read path must not choke on them.
--
-- `behavior_cell_code_ref_idx` is what makes the reverse lookup
-- (`GET /api/matrices/behaviors-for-file?path=`) an index scan rather than a
-- sequential one; it is not redundant with the cell index.

CREATE TABLE IF NOT EXISTS public.behavior_cell_code (
    id text NOT NULL,
    cell_id text NOT NULL,
    kind text NOT NULL,
    ref text NOT NULL,
    created_at timestamp without time zone DEFAULT now() NOT NULL
);

DO $$ BEGIN
    ALTER TABLE ONLY public.behavior_cell_code
        ADD CONSTRAINT behavior_cell_code_pkey PRIMARY KEY (id);
EXCEPTION WHEN duplicate_table OR invalid_table_definition THEN NULL;
END $$;

DO $$ BEGIN
    ALTER TABLE ONLY public.behavior_cell_code
        ADD CONSTRAINT behavior_cell_code_cell_kind_ref UNIQUE (cell_id, kind, ref);
EXCEPTION WHEN duplicate_table OR duplicate_object THEN NULL;
END $$;

DO $$ BEGIN
    ALTER TABLE ONLY public.behavior_cell_code
        ADD CONSTRAINT behavior_cell_code_cell_id_fkey
        FOREIGN KEY (cell_id) REFERENCES public.behavior_cell(id) ON DELETE CASCADE;
EXCEPTION WHEN duplicate_object THEN NULL;
END $$;

CREATE INDEX IF NOT EXISTS "behavior_cell_code_cellId_idx"
    ON public.behavior_cell_code USING btree (cell_id);

CREATE INDEX IF NOT EXISTS behavior_cell_code_ref_idx
    ON public.behavior_cell_code USING btree (ref);

-- ---------------------------------------------------------------------------
-- behavior_test_result: recorded pass/fail outcomes
-- ---------------------------------------------------------------------------
--
-- What turns a cell's status from "specified" into "verified" or "violated"
-- from real runs rather than a manually-toggled badge. `status` is
-- `pass | fail` as free text, same disposition as `behavior_cell_code.kind`.
-- Append-only: there is no unique constraint on (cell_id, test_ref), because
-- the history of runs for one test is the point.

CREATE TABLE IF NOT EXISTS public.behavior_test_result (
    id text NOT NULL,
    cell_id text NOT NULL,
    test_ref text NOT NULL,
    status text NOT NULL,
    detail text,
    run_at timestamp without time zone DEFAULT now() NOT NULL
);

DO $$ BEGIN
    ALTER TABLE ONLY public.behavior_test_result
        ADD CONSTRAINT behavior_test_result_pkey PRIMARY KEY (id);
EXCEPTION WHEN duplicate_table OR invalid_table_definition THEN NULL;
END $$;

DO $$ BEGIN
    ALTER TABLE ONLY public.behavior_test_result
        ADD CONSTRAINT behavior_test_result_cell_id_fkey
        FOREIGN KEY (cell_id) REFERENCES public.behavior_cell(id) ON DELETE CASCADE;
EXCEPTION WHEN duplicate_object THEN NULL;
END $$;

CREATE INDEX IF NOT EXISTS "behavior_test_result_cellId_idx"
    ON public.behavior_test_result USING btree (cell_id);
