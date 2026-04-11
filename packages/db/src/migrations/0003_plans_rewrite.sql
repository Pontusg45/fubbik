-- Plans rewrite: drop session tables and old plan sub-tables, create new plan schema
-- Task 1: Schema rewrite + migration

-- Drop session and old plan tables (CASCADE to handle FK constraints)
DROP TABLE IF EXISTS "session_requirement_ref" CASCADE;
DROP TABLE IF EXISTS "session_assumption" CASCADE;
DROP TABLE IF EXISTS "session_chunk_ref" CASCADE;
DROP TABLE IF EXISTS "implementation_session" CASCADE;
DROP TABLE IF EXISTS "plan_chunk_ref" CASCADE;
DROP TABLE IF EXISTS "plan_step" CASCADE;
DROP TABLE IF EXISTS "plan" CASCADE;

--> statement-breakpoint

-- Create new plan table
CREATE TABLE "plan" (
    "id" text PRIMARY KEY NOT NULL,
    "title" text NOT NULL,
    "description" text,
    "status" text NOT NULL DEFAULT 'draft',
    "user_id" text NOT NULL REFERENCES "user"("id") ON DELETE CASCADE,
    "codebase_id" text REFERENCES "codebase"("id") ON DELETE SET NULL,
    "created_at" timestamp DEFAULT now() NOT NULL,
    "updated_at" timestamp DEFAULT now() NOT NULL,
    "completed_at" timestamp
);
--> statement-breakpoint

CREATE INDEX "plan_userId_idx" ON "plan" ("user_id");
--> statement-breakpoint
CREATE INDEX "plan_codebaseId_idx" ON "plan" ("codebase_id");
--> statement-breakpoint

-- plan_requirement: link plans to requirements
CREATE TABLE "plan_requirement" (
    "id" text PRIMARY KEY NOT NULL,
    "plan_id" text NOT NULL REFERENCES "plan"("id") ON DELETE CASCADE,
    "requirement_id" text NOT NULL REFERENCES "requirement"("id") ON DELETE CASCADE,
    "order" integer NOT NULL DEFAULT 0,
    "created_at" timestamp DEFAULT now() NOT NULL
);
--> statement-breakpoint

CREATE UNIQUE INDEX "plan_requirement_unique_idx" ON "plan_requirement" ("plan_id", "requirement_id");
--> statement-breakpoint
CREATE INDEX "plan_requirement_planId_idx" ON "plan_requirement" ("plan_id");
--> statement-breakpoint

-- plan_analyze_item: discriminated analyze items (chunk, file, risk, assumption, question)
CREATE TABLE "plan_analyze_item" (
    "id" text PRIMARY KEY NOT NULL,
    "plan_id" text NOT NULL REFERENCES "plan"("id") ON DELETE CASCADE,
    "kind" text NOT NULL,
    "order" integer NOT NULL DEFAULT 0,
    "chunk_id" text REFERENCES "chunk"("id") ON DELETE CASCADE,
    "file_path" text,
    "text" text,
    "metadata" jsonb NOT NULL DEFAULT '{}',
    "created_at" timestamp DEFAULT now() NOT NULL,
    "updated_at" timestamp DEFAULT now() NOT NULL
);
--> statement-breakpoint

CREATE INDEX "plan_analyze_item_planId_idx" ON "plan_analyze_item" ("plan_id");
--> statement-breakpoint
CREATE INDEX "plan_analyze_item_chunkId_idx" ON "plan_analyze_item" ("chunk_id");
--> statement-breakpoint

-- plan_task: enriched tasks within a plan
CREATE TABLE "plan_task" (
    "id" text PRIMARY KEY NOT NULL,
    "plan_id" text NOT NULL REFERENCES "plan"("id") ON DELETE CASCADE,
    "title" text NOT NULL,
    "description" text,
    "acceptance_criteria" jsonb NOT NULL DEFAULT '[]',
    "status" text NOT NULL DEFAULT 'pending',
    "order" integer NOT NULL DEFAULT 0,
    "created_at" timestamp DEFAULT now() NOT NULL,
    "updated_at" timestamp DEFAULT now() NOT NULL
);
--> statement-breakpoint

CREATE INDEX "plan_task_planId_idx" ON "plan_task" ("plan_id");
--> statement-breakpoint

-- plan_task_chunk: multi-chunk links per task
CREATE TABLE "plan_task_chunk" (
    "id" text PRIMARY KEY NOT NULL,
    "task_id" text NOT NULL REFERENCES "plan_task"("id") ON DELETE CASCADE,
    "chunk_id" text NOT NULL REFERENCES "chunk"("id") ON DELETE CASCADE,
    "relation" text NOT NULL,
    "created_at" timestamp DEFAULT now() NOT NULL
);
--> statement-breakpoint

CREATE UNIQUE INDEX "plan_task_chunk_unique_idx" ON "plan_task_chunk" ("task_id", "chunk_id", "relation");
--> statement-breakpoint
CREATE INDEX "plan_task_chunk_taskId_idx" ON "plan_task_chunk" ("task_id");
--> statement-breakpoint

-- plan_task_dependency: task dependencies
CREATE TABLE "plan_task_dependency" (
    "id" text PRIMARY KEY NOT NULL,
    "task_id" text NOT NULL REFERENCES "plan_task"("id") ON DELETE CASCADE,
    "depends_on_task_id" text NOT NULL REFERENCES "plan_task"("id") ON DELETE CASCADE,
    "created_at" timestamp DEFAULT now() NOT NULL
);
--> statement-breakpoint

CREATE UNIQUE INDEX "plan_task_dependency_unique_idx" ON "plan_task_dependency" ("task_id", "depends_on_task_id");
--> statement-breakpoint
CREATE INDEX "plan_task_dependency_taskId_idx" ON "plan_task_dependency" ("task_id");
