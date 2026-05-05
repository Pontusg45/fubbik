CREATE TABLE "activity_log" (
	"id" text PRIMARY KEY NOT NULL,
	"user_id" text NOT NULL,
	"entity_type" text NOT NULL,
	"entity_id" text NOT NULL,
	"entity_title" text,
	"action" text NOT NULL,
	"codebase_id" text,
	"created_at" timestamp DEFAULT now() NOT NULL
);
--> statement-breakpoint
CREATE TABLE "chunk_applies_to" (
	"id" text PRIMARY KEY NOT NULL,
	"chunk_id" text NOT NULL,
	"pattern" text NOT NULL,
	"note" text
);
--> statement-breakpoint
CREATE TABLE "account" (
	"id" text PRIMARY KEY NOT NULL,
	"account_id" text NOT NULL,
	"provider_id" text NOT NULL,
	"user_id" text NOT NULL,
	"access_token" text,
	"refresh_token" text,
	"id_token" text,
	"access_token_expires_at" timestamp,
	"refresh_token_expires_at" timestamp,
	"scope" text,
	"password" text,
	"created_at" timestamp DEFAULT now() NOT NULL,
	"updated_at" timestamp NOT NULL
);
--> statement-breakpoint
CREATE TABLE "session" (
	"id" text PRIMARY KEY NOT NULL,
	"expires_at" timestamp NOT NULL,
	"token" text NOT NULL,
	"created_at" timestamp DEFAULT now() NOT NULL,
	"updated_at" timestamp NOT NULL,
	"ip_address" text,
	"user_agent" text,
	"user_id" text NOT NULL,
	CONSTRAINT "session_token_unique" UNIQUE("token")
);
--> statement-breakpoint
CREATE TABLE "user" (
	"id" text PRIMARY KEY NOT NULL,
	"name" text NOT NULL,
	"email" text NOT NULL,
	"email_verified" boolean DEFAULT false NOT NULL,
	"image" text,
	"created_at" timestamp DEFAULT now() NOT NULL,
	"updated_at" timestamp DEFAULT now() NOT NULL,
	CONSTRAINT "user_email_unique" UNIQUE("email")
);
--> statement-breakpoint
CREATE TABLE "verification" (
	"id" text PRIMARY KEY NOT NULL,
	"identifier" text NOT NULL,
	"value" text NOT NULL,
	"expires_at" timestamp NOT NULL,
	"created_at" timestamp DEFAULT now() NOT NULL,
	"updated_at" timestamp DEFAULT now() NOT NULL
);
--> statement-breakpoint
CREATE TABLE "chunk_proposal" (
	"id" text PRIMARY KEY NOT NULL,
	"chunk_id" text NOT NULL,
	"changes" jsonb NOT NULL,
	"reason" text,
	"status" text DEFAULT 'pending' NOT NULL,
	"proposed_by" text NOT NULL,
	"reviewed_by" text,
	"reviewed_at" timestamp,
	"review_note" text,
	"created_at" timestamp DEFAULT now() NOT NULL
);
--> statement-breakpoint
CREATE TABLE "chunk_type" (
	"id" text PRIMARY KEY NOT NULL,
	"label" text NOT NULL,
	"description" text,
	"icon" text,
	"color" text DEFAULT '#8b5cf6' NOT NULL,
	"examples" jsonb DEFAULT '[]'::jsonb NOT NULL,
	"display_order" integer DEFAULT 100 NOT NULL,
	"built_in" boolean DEFAULT false NOT NULL,
	"user_id" text,
	"codebase_id" text,
	"created_at" timestamp DEFAULT now() NOT NULL,
	"updated_at" timestamp DEFAULT now() NOT NULL
);
--> statement-breakpoint
CREATE TABLE "chunk_version" (
	"id" text PRIMARY KEY NOT NULL,
	"chunk_id" text NOT NULL,
	"version" integer NOT NULL,
	"title" text NOT NULL,
	"content" text NOT NULL,
	"type" text NOT NULL,
	"tags" jsonb NOT NULL,
	"created_at" timestamp DEFAULT now() NOT NULL
);
--> statement-breakpoint
CREATE TABLE "chunk" (
	"id" text PRIMARY KEY NOT NULL,
	"title" text NOT NULL,
	"content" text DEFAULT '' NOT NULL,
	"type" text DEFAULT 'note' NOT NULL,
	"user_id" text NOT NULL,
	"created_at" timestamp DEFAULT now() NOT NULL,
	"updated_at" timestamp DEFAULT now() NOT NULL,
	"summary" text,
	"aliases" jsonb DEFAULT '[]'::jsonb NOT NULL,
	"not_about" jsonb DEFAULT '[]'::jsonb NOT NULL,
	"scope" jsonb DEFAULT '{}'::jsonb NOT NULL,
	"rationale" text,
	"alternatives" jsonb,
	"consequences" text,
	"embedding" vector(768),
	"embedding_updated_at" timestamp,
	"origin" text DEFAULT 'human' NOT NULL,
	"review_status" text DEFAULT 'approved' NOT NULL,
	"reviewed_by" text,
	"reviewed_at" timestamp,
	"archived_at" timestamp,
	"document_id" text,
	"document_order" integer,
	"is_entry_point" boolean DEFAULT false NOT NULL
);
--> statement-breakpoint
CREATE TABLE "chunk_connection" (
	"id" text PRIMARY KEY NOT NULL,
	"source_id" text NOT NULL,
	"target_id" text NOT NULL,
	"relation" text DEFAULT 'related_to' NOT NULL,
	"created_at" timestamp DEFAULT now() NOT NULL,
	"origin" text DEFAULT 'human' NOT NULL,
	"review_status" text DEFAULT 'approved' NOT NULL,
	"reviewed_by" text,
	"reviewed_at" timestamp
);
--> statement-breakpoint
CREATE TABLE "chunk_codebase" (
	"chunk_id" text NOT NULL,
	"codebase_id" text NOT NULL,
	CONSTRAINT "chunk_codebase_chunk_id_codebase_id_pk" PRIMARY KEY("chunk_id","codebase_id")
);
--> statement-breakpoint
CREATE TABLE "codebase" (
	"id" text PRIMARY KEY NOT NULL,
	"name" text NOT NULL,
	"remote_url" text,
	"local_paths" jsonb DEFAULT '[]'::jsonb NOT NULL,
	"user_id" text NOT NULL,
	"created_at" timestamp DEFAULT now() NOT NULL,
	"updated_at" timestamp DEFAULT now() NOT NULL
);
--> statement-breakpoint
CREATE TABLE "collection" (
	"id" text PRIMARY KEY NOT NULL,
	"name" text NOT NULL,
	"description" text,
	"filter" jsonb NOT NULL,
	"user_id" text NOT NULL,
	"codebase_id" text,
	"created_at" timestamp DEFAULT now() NOT NULL,
	"updated_at" timestamp DEFAULT now() NOT NULL
);
--> statement-breakpoint
CREATE TABLE "chunk_comment" (
	"id" text PRIMARY KEY NOT NULL,
	"chunk_id" text NOT NULL,
	"user_id" text NOT NULL,
	"content" text NOT NULL,
	"created_at" timestamp DEFAULT now() NOT NULL,
	"updated_at" timestamp DEFAULT now() NOT NULL
);
--> statement-breakpoint
CREATE TABLE "connection_relation" (
	"id" text PRIMARY KEY NOT NULL,
	"label" text NOT NULL,
	"description" text,
	"arrow_style" text DEFAULT 'solid' NOT NULL,
	"direction" text DEFAULT 'forward' NOT NULL,
	"color" text DEFAULT '#64748b' NOT NULL,
	"inverse_of_id" text,
	"display_order" integer DEFAULT 100 NOT NULL,
	"built_in" boolean DEFAULT false NOT NULL,
	"user_id" text,
	"codebase_id" text,
	"created_at" timestamp DEFAULT now() NOT NULL,
	"updated_at" timestamp DEFAULT now() NOT NULL
);
--> statement-breakpoint
CREATE TABLE "context_snapshot" (
	"id" text PRIMARY KEY NOT NULL,
	"user_id" text NOT NULL,
	"query" jsonb NOT NULL,
	"chunks" jsonb NOT NULL,
	"token_count" integer NOT NULL,
	"created_at" timestamp DEFAULT now() NOT NULL
);
--> statement-breakpoint
CREATE TABLE "document" (
	"id" text PRIMARY KEY NOT NULL,
	"title" text NOT NULL,
	"source_path" text NOT NULL,
	"content_hash" text NOT NULL,
	"description" text,
	"codebase_id" text,
	"user_id" text NOT NULL,
	"created_at" timestamp DEFAULT now() NOT NULL,
	"updated_at" timestamp DEFAULT now() NOT NULL
);
--> statement-breakpoint
CREATE TABLE "user_favorite" (
	"id" text PRIMARY KEY NOT NULL,
	"user_id" text NOT NULL,
	"chunk_id" text NOT NULL,
	"order" integer DEFAULT 0 NOT NULL,
	"created_at" timestamp DEFAULT now() NOT NULL
);
--> statement-breakpoint
CREATE TABLE "chunk_feature_delta" (
	"id" text PRIMARY KEY NOT NULL,
	"chunk_id" text NOT NULL,
	"feature_id" text NOT NULL,
	"delta" jsonb NOT NULL,
	"created_at" timestamp DEFAULT now() NOT NULL,
	"updated_at" timestamp DEFAULT now() NOT NULL
);
--> statement-breakpoint
CREATE TABLE "feature" (
	"id" text PRIMARY KEY NOT NULL,
	"name" text NOT NULL,
	"description" text,
	"priority" integer NOT NULL,
	"status" text DEFAULT 'inactive' NOT NULL,
	"color" text,
	"user_id" text NOT NULL,
	"created_at" timestamp DEFAULT now() NOT NULL,
	"updated_at" timestamp DEFAULT now() NOT NULL
);
--> statement-breakpoint
CREATE TABLE "feature_codebase" (
	"feature_id" text NOT NULL,
	"codebase_id" text NOT NULL,
	CONSTRAINT "feature_codebase_feature_id_codebase_id_pk" PRIMARY KEY("feature_id","codebase_id")
);
--> statement-breakpoint
CREATE TABLE "user_active_feature" (
	"user_id" text NOT NULL,
	"feature_id" text NOT NULL,
	CONSTRAINT "user_active_feature_user_id_feature_id_pk" PRIMARY KEY("user_id","feature_id")
);
--> statement-breakpoint
CREATE TABLE "chunk_file_ref" (
	"id" text PRIMARY KEY NOT NULL,
	"chunk_id" text NOT NULL,
	"path" text NOT NULL,
	"anchor" text,
	"relation" text DEFAULT 'documents' NOT NULL
);
--> statement-breakpoint
CREATE TABLE "chunk_tag" (
	"chunk_id" text NOT NULL,
	"tag_id" text NOT NULL,
	CONSTRAINT "chunk_tag_chunk_id_tag_id_pk" PRIMARY KEY("chunk_id","tag_id")
);
--> statement-breakpoint
CREATE TABLE "tag" (
	"id" text PRIMARY KEY NOT NULL,
	"name" text NOT NULL,
	"tag_type_id" text,
	"user_id" text NOT NULL,
	"created_at" timestamp DEFAULT now() NOT NULL,
	"origin" text DEFAULT 'human' NOT NULL,
	"review_status" text DEFAULT 'approved' NOT NULL,
	"reviewed_by" text,
	"reviewed_at" timestamp
);
--> statement-breakpoint
CREATE TABLE "tag_type" (
	"id" text PRIMARY KEY NOT NULL,
	"name" text NOT NULL,
	"color" text DEFAULT '#8b5cf6' NOT NULL,
	"icon" text,
	"user_id" text NOT NULL,
	"created_at" timestamp DEFAULT now() NOT NULL
);
--> statement-breakpoint
CREATE TABLE "chunk_template" (
	"id" text PRIMARY KEY NOT NULL,
	"name" text NOT NULL,
	"description" text,
	"type" text DEFAULT 'note' NOT NULL,
	"content" text DEFAULT '' NOT NULL,
	"is_built_in" boolean DEFAULT false NOT NULL,
	"match_rules" jsonb,
	"field_mappings" jsonb,
	"priority" integer DEFAULT 0 NOT NULL,
	"tags" text[],
	"user_id" text,
	"created_at" timestamp DEFAULT now() NOT NULL
);
--> statement-breakpoint
CREATE TABLE "use_case" (
	"id" text PRIMARY KEY NOT NULL,
	"name" text NOT NULL,
	"description" text,
	"codebase_id" text,
	"user_id" text NOT NULL,
	"order" integer DEFAULT 0 NOT NULL,
	"parent_id" text,
	"created_at" timestamp DEFAULT now() NOT NULL,
	"updated_at" timestamp DEFAULT now() NOT NULL
);
--> statement-breakpoint
CREATE TABLE "requirement" (
	"id" text PRIMARY KEY NOT NULL,
	"title" text NOT NULL,
	"description" text,
	"steps" jsonb NOT NULL,
	"order" integer DEFAULT 0 NOT NULL,
	"status" text DEFAULT 'untested' NOT NULL,
	"priority" text,
	"codebase_id" text,
	"user_id" text NOT NULL,
	"created_at" timestamp DEFAULT now() NOT NULL,
	"updated_at" timestamp DEFAULT now() NOT NULL,
	"origin" text DEFAULT 'human' NOT NULL,
	"review_status" text DEFAULT 'approved' NOT NULL,
	"use_case_id" text,
	"reviewed_by" text,
	"reviewed_at" timestamp
);
--> statement-breakpoint
CREATE TABLE "requirement_chunk" (
	"requirement_id" text NOT NULL,
	"chunk_id" text NOT NULL,
	CONSTRAINT "requirement_chunk_requirement_id_chunk_id_pk" PRIMARY KEY("requirement_id","chunk_id")
);
--> statement-breakpoint
CREATE TABLE "requirement_dependency" (
	"requirement_id" text NOT NULL,
	"depends_on_id" text NOT NULL,
	CONSTRAINT "requirement_dependency_requirement_id_depends_on_id_pk" PRIMARY KEY("requirement_id","depends_on_id"),
	CONSTRAINT "no_self_dependency" CHECK ("requirement_dependency"."requirement_id" != "requirement_dependency"."depends_on_id")
);
--> statement-breakpoint
CREATE TABLE "vocabulary_entry" (
	"id" text PRIMARY KEY NOT NULL,
	"word" text NOT NULL,
	"category" text NOT NULL,
	"expects" jsonb,
	"codebase_id" text NOT NULL,
	"user_id" text,
	"created_at" timestamp DEFAULT now() NOT NULL,
	"updated_at" timestamp DEFAULT now() NOT NULL
);
--> statement-breakpoint
CREATE TABLE "notification" (
	"id" text PRIMARY KEY NOT NULL,
	"user_id" text NOT NULL,
	"type" text NOT NULL,
	"title" text NOT NULL,
	"message" text NOT NULL,
	"link_to" text,
	"read" boolean DEFAULT false NOT NULL,
	"created_at" timestamp DEFAULT now() NOT NULL
);
--> statement-breakpoint
CREATE TABLE "codebase_settings" (
	"id" text PRIMARY KEY NOT NULL,
	"codebase_id" text NOT NULL,
	"key" text NOT NULL,
	"value" jsonb NOT NULL,
	"updated_at" timestamp DEFAULT now() NOT NULL
);
--> statement-breakpoint
CREATE TABLE "instance_settings" (
	"key" text PRIMARY KEY NOT NULL,
	"value" jsonb NOT NULL,
	"updated_at" timestamp DEFAULT now() NOT NULL
);
--> statement-breakpoint
CREATE TABLE "user_settings" (
	"id" text PRIMARY KEY NOT NULL,
	"user_id" text NOT NULL,
	"key" text NOT NULL,
	"value" jsonb NOT NULL,
	"updated_at" timestamp DEFAULT now() NOT NULL
);
--> statement-breakpoint
CREATE TABLE "plan" (
	"id" text PRIMARY KEY NOT NULL,
	"title" text NOT NULL,
	"description" text,
	"status" text DEFAULT 'draft' NOT NULL,
	"user_id" text NOT NULL,
	"codebase_id" text,
	"created_at" timestamp DEFAULT now() NOT NULL,
	"updated_at" timestamp DEFAULT now() NOT NULL,
	"completed_at" timestamp,
	"metadata" jsonb DEFAULT '{}'::jsonb NOT NULL
);
--> statement-breakpoint
CREATE TABLE "plan_analyze_item" (
	"id" text PRIMARY KEY NOT NULL,
	"plan_id" text NOT NULL,
	"kind" text NOT NULL,
	"order" integer DEFAULT 0 NOT NULL,
	"chunk_id" text,
	"file_path" text,
	"text" text,
	"metadata" jsonb DEFAULT '{}'::jsonb NOT NULL,
	"created_at" timestamp DEFAULT now() NOT NULL,
	"updated_at" timestamp DEFAULT now() NOT NULL
);
--> statement-breakpoint
CREATE TABLE "plan_external_link" (
	"id" text PRIMARY KEY NOT NULL,
	"plan_id" text NOT NULL,
	"system" text DEFAULT 'url' NOT NULL,
	"url" text NOT NULL,
	"label" text,
	"order" integer DEFAULT 0 NOT NULL,
	"created_at" timestamp DEFAULT now() NOT NULL
);
--> statement-breakpoint
CREATE TABLE "plan_requirement" (
	"id" text PRIMARY KEY NOT NULL,
	"plan_id" text NOT NULL,
	"requirement_id" text NOT NULL,
	"order" integer DEFAULT 0 NOT NULL,
	"created_at" timestamp DEFAULT now() NOT NULL
);
--> statement-breakpoint
CREATE TABLE "plan_task" (
	"id" text PRIMARY KEY NOT NULL,
	"plan_id" text NOT NULL,
	"title" text NOT NULL,
	"description" text,
	"acceptance_criteria" jsonb DEFAULT '[]'::jsonb NOT NULL,
	"status" text DEFAULT 'pending' NOT NULL,
	"order" integer DEFAULT 0 NOT NULL,
	"created_at" timestamp DEFAULT now() NOT NULL,
	"updated_at" timestamp DEFAULT now() NOT NULL,
	"metadata" jsonb DEFAULT '{}'::jsonb NOT NULL
);
--> statement-breakpoint
CREATE TABLE "plan_task_chunk" (
	"id" text PRIMARY KEY NOT NULL,
	"task_id" text NOT NULL,
	"chunk_id" text NOT NULL,
	"relation" text NOT NULL,
	"created_at" timestamp DEFAULT now() NOT NULL
);
--> statement-breakpoint
CREATE TABLE "plan_task_dependency" (
	"id" text PRIMARY KEY NOT NULL,
	"task_id" text NOT NULL,
	"depends_on_task_id" text NOT NULL,
	"created_at" timestamp DEFAULT now() NOT NULL
);
--> statement-breakpoint
CREATE TABLE "plan_task_external_link" (
	"id" text PRIMARY KEY NOT NULL,
	"task_id" text NOT NULL,
	"system" text DEFAULT 'url' NOT NULL,
	"url" text NOT NULL,
	"label" text,
	"order" integer DEFAULT 0 NOT NULL,
	"created_at" timestamp DEFAULT now() NOT NULL
);
--> statement-breakpoint
CREATE TABLE "workspace" (
	"id" text PRIMARY KEY NOT NULL,
	"name" text NOT NULL,
	"description" text,
	"user_id" text NOT NULL,
	"created_at" timestamp DEFAULT now() NOT NULL,
	"updated_at" timestamp DEFAULT now() NOT NULL
);
--> statement-breakpoint
CREATE TABLE "workspace_codebase" (
	"workspace_id" text NOT NULL,
	"codebase_id" text NOT NULL,
	CONSTRAINT "workspace_codebase_workspace_id_codebase_id_pk" PRIMARY KEY("workspace_id","codebase_id")
);
--> statement-breakpoint
CREATE TABLE "saved_graph" (
	"id" text PRIMARY KEY NOT NULL,
	"name" text NOT NULL,
	"description" text,
	"chunk_ids" jsonb DEFAULT '[]'::jsonb NOT NULL,
	"positions" jsonb DEFAULT '{}'::jsonb NOT NULL,
	"layout_algorithm" text DEFAULT 'force' NOT NULL,
	"user_id" text NOT NULL,
	"codebase_id" text,
	"created_at" timestamp DEFAULT now() NOT NULL,
	"updated_at" timestamp DEFAULT now() NOT NULL
);
--> statement-breakpoint
CREATE TABLE "saved_query" (
	"id" text PRIMARY KEY NOT NULL,
	"name" text NOT NULL,
	"query" jsonb NOT NULL,
	"user_id" text NOT NULL,
	"codebase_id" text,
	"created_at" timestamp DEFAULT now() NOT NULL
);
--> statement-breakpoint
CREATE TABLE "chunk_staleness" (
	"id" text PRIMARY KEY NOT NULL,
	"chunk_id" text NOT NULL,
	"reason" text NOT NULL,
	"detail" text,
	"related_chunk_id" text,
	"detected_at" timestamp DEFAULT now() NOT NULL,
	"dismissed_at" timestamp,
	"dismissed_by" text,
	"suppress_pair" text
);
--> statement-breakpoint
CREATE TABLE "staleness_scan" (
	"id" text PRIMARY KEY NOT NULL,
	"codebase_id" text NOT NULL,
	"last_commit_sha" text NOT NULL,
	"scanned_at" timestamp DEFAULT now() NOT NULL
);
--> statement-breakpoint
CREATE TABLE "learning_path" (
	"id" text PRIMARY KEY NOT NULL,
	"title" text NOT NULL,
	"description" text,
	"chunk_ids" jsonb DEFAULT '[]'::jsonb NOT NULL,
	"user_id" text NOT NULL,
	"created_at" timestamp DEFAULT now() NOT NULL,
	"updated_at" timestamp DEFAULT now() NOT NULL
);
--> statement-breakpoint
CREATE TABLE "scope_key" (
	"id" text PRIMARY KEY NOT NULL,
	"user_id" text NOT NULL,
	"key" text NOT NULL,
	"description" text,
	"value_type" text DEFAULT 'string' NOT NULL,
	"allowed_values" jsonb,
	"created_at" timestamp DEFAULT now() NOT NULL,
	CONSTRAINT "scope_key_user_key_unique" UNIQUE("user_id","key")
);
--> statement-breakpoint
ALTER TABLE "activity_log" ADD CONSTRAINT "activity_log_user_id_user_id_fk" FOREIGN KEY ("user_id") REFERENCES "public"."user"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "activity_log" ADD CONSTRAINT "activity_log_codebase_id_codebase_id_fk" FOREIGN KEY ("codebase_id") REFERENCES "public"."codebase"("id") ON DELETE set null ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "chunk_applies_to" ADD CONSTRAINT "chunk_applies_to_chunk_id_chunk_id_fk" FOREIGN KEY ("chunk_id") REFERENCES "public"."chunk"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "account" ADD CONSTRAINT "account_user_id_user_id_fk" FOREIGN KEY ("user_id") REFERENCES "public"."user"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "session" ADD CONSTRAINT "session_user_id_user_id_fk" FOREIGN KEY ("user_id") REFERENCES "public"."user"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "chunk_proposal" ADD CONSTRAINT "chunk_proposal_chunk_id_chunk_id_fk" FOREIGN KEY ("chunk_id") REFERENCES "public"."chunk"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "chunk_proposal" ADD CONSTRAINT "chunk_proposal_reviewed_by_user_id_fk" FOREIGN KEY ("reviewed_by") REFERENCES "public"."user"("id") ON DELETE set null ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "chunk_type" ADD CONSTRAINT "chunk_type_user_id_user_id_fk" FOREIGN KEY ("user_id") REFERENCES "public"."user"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "chunk_type" ADD CONSTRAINT "chunk_type_codebase_id_codebase_id_fk" FOREIGN KEY ("codebase_id") REFERENCES "public"."codebase"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "chunk_version" ADD CONSTRAINT "chunk_version_chunk_id_chunk_id_fk" FOREIGN KEY ("chunk_id") REFERENCES "public"."chunk"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "chunk" ADD CONSTRAINT "chunk_type_chunk_type_id_fk" FOREIGN KEY ("type") REFERENCES "public"."chunk_type"("id") ON DELETE restrict ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "chunk" ADD CONSTRAINT "chunk_user_id_user_id_fk" FOREIGN KEY ("user_id") REFERENCES "public"."user"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "chunk" ADD CONSTRAINT "chunk_reviewed_by_user_id_fk" FOREIGN KEY ("reviewed_by") REFERENCES "public"."user"("id") ON DELETE set null ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "chunk" ADD CONSTRAINT "chunk_document_id_document_id_fk" FOREIGN KEY ("document_id") REFERENCES "public"."document"("id") ON DELETE set null ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "chunk_connection" ADD CONSTRAINT "chunk_connection_source_id_chunk_id_fk" FOREIGN KEY ("source_id") REFERENCES "public"."chunk"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "chunk_connection" ADD CONSTRAINT "chunk_connection_target_id_chunk_id_fk" FOREIGN KEY ("target_id") REFERENCES "public"."chunk"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "chunk_connection" ADD CONSTRAINT "chunk_connection_relation_connection_relation_id_fk" FOREIGN KEY ("relation") REFERENCES "public"."connection_relation"("id") ON DELETE restrict ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "chunk_connection" ADD CONSTRAINT "chunk_connection_reviewed_by_user_id_fk" FOREIGN KEY ("reviewed_by") REFERENCES "public"."user"("id") ON DELETE set null ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "chunk_codebase" ADD CONSTRAINT "chunk_codebase_chunk_id_chunk_id_fk" FOREIGN KEY ("chunk_id") REFERENCES "public"."chunk"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "chunk_codebase" ADD CONSTRAINT "chunk_codebase_codebase_id_codebase_id_fk" FOREIGN KEY ("codebase_id") REFERENCES "public"."codebase"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "codebase" ADD CONSTRAINT "codebase_user_id_user_id_fk" FOREIGN KEY ("user_id") REFERENCES "public"."user"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "collection" ADD CONSTRAINT "collection_user_id_user_id_fk" FOREIGN KEY ("user_id") REFERENCES "public"."user"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "collection" ADD CONSTRAINT "collection_codebase_id_codebase_id_fk" FOREIGN KEY ("codebase_id") REFERENCES "public"."codebase"("id") ON DELETE set null ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "chunk_comment" ADD CONSTRAINT "chunk_comment_chunk_id_chunk_id_fk" FOREIGN KEY ("chunk_id") REFERENCES "public"."chunk"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "chunk_comment" ADD CONSTRAINT "chunk_comment_user_id_user_id_fk" FOREIGN KEY ("user_id") REFERENCES "public"."user"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "connection_relation" ADD CONSTRAINT "connection_relation_inverse_of_id_connection_relation_id_fk" FOREIGN KEY ("inverse_of_id") REFERENCES "public"."connection_relation"("id") ON DELETE set null ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "connection_relation" ADD CONSTRAINT "connection_relation_user_id_user_id_fk" FOREIGN KEY ("user_id") REFERENCES "public"."user"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "connection_relation" ADD CONSTRAINT "connection_relation_codebase_id_codebase_id_fk" FOREIGN KEY ("codebase_id") REFERENCES "public"."codebase"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "context_snapshot" ADD CONSTRAINT "context_snapshot_user_id_user_id_fk" FOREIGN KEY ("user_id") REFERENCES "public"."user"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "document" ADD CONSTRAINT "document_codebase_id_codebase_id_fk" FOREIGN KEY ("codebase_id") REFERENCES "public"."codebase"("id") ON DELETE set null ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "document" ADD CONSTRAINT "document_user_id_user_id_fk" FOREIGN KEY ("user_id") REFERENCES "public"."user"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "user_favorite" ADD CONSTRAINT "user_favorite_user_id_user_id_fk" FOREIGN KEY ("user_id") REFERENCES "public"."user"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "user_favorite" ADD CONSTRAINT "user_favorite_chunk_id_chunk_id_fk" FOREIGN KEY ("chunk_id") REFERENCES "public"."chunk"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "chunk_feature_delta" ADD CONSTRAINT "chunk_feature_delta_chunk_id_chunk_id_fk" FOREIGN KEY ("chunk_id") REFERENCES "public"."chunk"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "chunk_feature_delta" ADD CONSTRAINT "chunk_feature_delta_feature_id_feature_id_fk" FOREIGN KEY ("feature_id") REFERENCES "public"."feature"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "feature" ADD CONSTRAINT "feature_user_id_user_id_fk" FOREIGN KEY ("user_id") REFERENCES "public"."user"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "feature_codebase" ADD CONSTRAINT "feature_codebase_feature_id_feature_id_fk" FOREIGN KEY ("feature_id") REFERENCES "public"."feature"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "feature_codebase" ADD CONSTRAINT "feature_codebase_codebase_id_codebase_id_fk" FOREIGN KEY ("codebase_id") REFERENCES "public"."codebase"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "user_active_feature" ADD CONSTRAINT "user_active_feature_user_id_user_id_fk" FOREIGN KEY ("user_id") REFERENCES "public"."user"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "user_active_feature" ADD CONSTRAINT "user_active_feature_feature_id_feature_id_fk" FOREIGN KEY ("feature_id") REFERENCES "public"."feature"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "chunk_file_ref" ADD CONSTRAINT "chunk_file_ref_chunk_id_chunk_id_fk" FOREIGN KEY ("chunk_id") REFERENCES "public"."chunk"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "chunk_tag" ADD CONSTRAINT "chunk_tag_chunk_id_chunk_id_fk" FOREIGN KEY ("chunk_id") REFERENCES "public"."chunk"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "chunk_tag" ADD CONSTRAINT "chunk_tag_tag_id_tag_id_fk" FOREIGN KEY ("tag_id") REFERENCES "public"."tag"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "tag" ADD CONSTRAINT "tag_tag_type_id_tag_type_id_fk" FOREIGN KEY ("tag_type_id") REFERENCES "public"."tag_type"("id") ON DELETE set null ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "tag" ADD CONSTRAINT "tag_user_id_user_id_fk" FOREIGN KEY ("user_id") REFERENCES "public"."user"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "tag" ADD CONSTRAINT "tag_reviewed_by_user_id_fk" FOREIGN KEY ("reviewed_by") REFERENCES "public"."user"("id") ON DELETE set null ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "tag_type" ADD CONSTRAINT "tag_type_user_id_user_id_fk" FOREIGN KEY ("user_id") REFERENCES "public"."user"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "chunk_template" ADD CONSTRAINT "chunk_template_user_id_user_id_fk" FOREIGN KEY ("user_id") REFERENCES "public"."user"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "use_case" ADD CONSTRAINT "use_case_codebase_id_codebase_id_fk" FOREIGN KEY ("codebase_id") REFERENCES "public"."codebase"("id") ON DELETE set null ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "use_case" ADD CONSTRAINT "use_case_user_id_user_id_fk" FOREIGN KEY ("user_id") REFERENCES "public"."user"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "use_case" ADD CONSTRAINT "use_case_parent_id_use_case_id_fk" FOREIGN KEY ("parent_id") REFERENCES "public"."use_case"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "requirement" ADD CONSTRAINT "requirement_codebase_id_codebase_id_fk" FOREIGN KEY ("codebase_id") REFERENCES "public"."codebase"("id") ON DELETE set null ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "requirement" ADD CONSTRAINT "requirement_user_id_user_id_fk" FOREIGN KEY ("user_id") REFERENCES "public"."user"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "requirement" ADD CONSTRAINT "requirement_use_case_id_use_case_id_fk" FOREIGN KEY ("use_case_id") REFERENCES "public"."use_case"("id") ON DELETE set null ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "requirement" ADD CONSTRAINT "requirement_reviewed_by_user_id_fk" FOREIGN KEY ("reviewed_by") REFERENCES "public"."user"("id") ON DELETE set null ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "requirement_chunk" ADD CONSTRAINT "requirement_chunk_requirement_id_requirement_id_fk" FOREIGN KEY ("requirement_id") REFERENCES "public"."requirement"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "requirement_chunk" ADD CONSTRAINT "requirement_chunk_chunk_id_chunk_id_fk" FOREIGN KEY ("chunk_id") REFERENCES "public"."chunk"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "requirement_dependency" ADD CONSTRAINT "requirement_dependency_requirement_id_requirement_id_fk" FOREIGN KEY ("requirement_id") REFERENCES "public"."requirement"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "requirement_dependency" ADD CONSTRAINT "requirement_dependency_depends_on_id_requirement_id_fk" FOREIGN KEY ("depends_on_id") REFERENCES "public"."requirement"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "vocabulary_entry" ADD CONSTRAINT "vocabulary_entry_codebase_id_codebase_id_fk" FOREIGN KEY ("codebase_id") REFERENCES "public"."codebase"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "vocabulary_entry" ADD CONSTRAINT "vocabulary_entry_user_id_user_id_fk" FOREIGN KEY ("user_id") REFERENCES "public"."user"("id") ON DELETE set null ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "notification" ADD CONSTRAINT "notification_user_id_user_id_fk" FOREIGN KEY ("user_id") REFERENCES "public"."user"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "codebase_settings" ADD CONSTRAINT "codebase_settings_codebase_id_codebase_id_fk" FOREIGN KEY ("codebase_id") REFERENCES "public"."codebase"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "user_settings" ADD CONSTRAINT "user_settings_user_id_user_id_fk" FOREIGN KEY ("user_id") REFERENCES "public"."user"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "plan" ADD CONSTRAINT "plan_user_id_user_id_fk" FOREIGN KEY ("user_id") REFERENCES "public"."user"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "plan" ADD CONSTRAINT "plan_codebase_id_codebase_id_fk" FOREIGN KEY ("codebase_id") REFERENCES "public"."codebase"("id") ON DELETE set null ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "plan_analyze_item" ADD CONSTRAINT "plan_analyze_item_plan_id_plan_id_fk" FOREIGN KEY ("plan_id") REFERENCES "public"."plan"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "plan_analyze_item" ADD CONSTRAINT "plan_analyze_item_chunk_id_chunk_id_fk" FOREIGN KEY ("chunk_id") REFERENCES "public"."chunk"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "plan_external_link" ADD CONSTRAINT "plan_external_link_plan_id_plan_id_fk" FOREIGN KEY ("plan_id") REFERENCES "public"."plan"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "plan_requirement" ADD CONSTRAINT "plan_requirement_plan_id_plan_id_fk" FOREIGN KEY ("plan_id") REFERENCES "public"."plan"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "plan_requirement" ADD CONSTRAINT "plan_requirement_requirement_id_requirement_id_fk" FOREIGN KEY ("requirement_id") REFERENCES "public"."requirement"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "plan_task" ADD CONSTRAINT "plan_task_plan_id_plan_id_fk" FOREIGN KEY ("plan_id") REFERENCES "public"."plan"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "plan_task_chunk" ADD CONSTRAINT "plan_task_chunk_task_id_plan_task_id_fk" FOREIGN KEY ("task_id") REFERENCES "public"."plan_task"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "plan_task_chunk" ADD CONSTRAINT "plan_task_chunk_chunk_id_chunk_id_fk" FOREIGN KEY ("chunk_id") REFERENCES "public"."chunk"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "plan_task_dependency" ADD CONSTRAINT "plan_task_dependency_task_id_plan_task_id_fk" FOREIGN KEY ("task_id") REFERENCES "public"."plan_task"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "plan_task_dependency" ADD CONSTRAINT "plan_task_dependency_depends_on_task_id_plan_task_id_fk" FOREIGN KEY ("depends_on_task_id") REFERENCES "public"."plan_task"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "plan_task_external_link" ADD CONSTRAINT "plan_task_external_link_task_id_plan_task_id_fk" FOREIGN KEY ("task_id") REFERENCES "public"."plan_task"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "workspace" ADD CONSTRAINT "workspace_user_id_user_id_fk" FOREIGN KEY ("user_id") REFERENCES "public"."user"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "workspace_codebase" ADD CONSTRAINT "workspace_codebase_workspace_id_workspace_id_fk" FOREIGN KEY ("workspace_id") REFERENCES "public"."workspace"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "workspace_codebase" ADD CONSTRAINT "workspace_codebase_codebase_id_codebase_id_fk" FOREIGN KEY ("codebase_id") REFERENCES "public"."codebase"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "saved_graph" ADD CONSTRAINT "saved_graph_user_id_user_id_fk" FOREIGN KEY ("user_id") REFERENCES "public"."user"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "saved_graph" ADD CONSTRAINT "saved_graph_codebase_id_codebase_id_fk" FOREIGN KEY ("codebase_id") REFERENCES "public"."codebase"("id") ON DELETE set null ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "saved_query" ADD CONSTRAINT "saved_query_user_id_user_id_fk" FOREIGN KEY ("user_id") REFERENCES "public"."user"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "saved_query" ADD CONSTRAINT "saved_query_codebase_id_codebase_id_fk" FOREIGN KEY ("codebase_id") REFERENCES "public"."codebase"("id") ON DELETE set null ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "chunk_staleness" ADD CONSTRAINT "chunk_staleness_chunk_id_chunk_id_fk" FOREIGN KEY ("chunk_id") REFERENCES "public"."chunk"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "chunk_staleness" ADD CONSTRAINT "chunk_staleness_related_chunk_id_chunk_id_fk" FOREIGN KEY ("related_chunk_id") REFERENCES "public"."chunk"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "chunk_staleness" ADD CONSTRAINT "chunk_staleness_dismissed_by_user_id_fk" FOREIGN KEY ("dismissed_by") REFERENCES "public"."user"("id") ON DELETE set null ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "staleness_scan" ADD CONSTRAINT "staleness_scan_codebase_id_codebase_id_fk" FOREIGN KEY ("codebase_id") REFERENCES "public"."codebase"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "learning_path" ADD CONSTRAINT "learning_path_user_id_user_id_fk" FOREIGN KEY ("user_id") REFERENCES "public"."user"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "scope_key" ADD CONSTRAINT "scope_key_user_id_user_id_fk" FOREIGN KEY ("user_id") REFERENCES "public"."user"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
CREATE INDEX "activity_userId_idx" ON "activity_log" USING btree ("user_id");--> statement-breakpoint
CREATE INDEX "activity_createdAt_idx" ON "activity_log" USING btree ("created_at");--> statement-breakpoint
CREATE INDEX "activity_codebaseId_idx" ON "activity_log" USING btree ("codebase_id");--> statement-breakpoint
CREATE INDEX "chunk_applies_to_chunkId_idx" ON "chunk_applies_to" USING btree ("chunk_id");--> statement-breakpoint
CREATE INDEX "account_userId_idx" ON "account" USING btree ("user_id");--> statement-breakpoint
CREATE INDEX "session_userId_idx" ON "session" USING btree ("user_id");--> statement-breakpoint
CREATE INDEX "verification_identifier_idx" ON "verification" USING btree ("identifier");--> statement-breakpoint
CREATE INDEX "chunk_proposal_chunkId_idx" ON "chunk_proposal" USING btree ("chunk_id");--> statement-breakpoint
CREATE INDEX "chunk_proposal_status_idx" ON "chunk_proposal" USING btree ("status");--> statement-breakpoint
CREATE INDEX "chunk_proposal_chunkId_status_idx" ON "chunk_proposal" USING btree ("chunk_id","status");--> statement-breakpoint
CREATE UNIQUE INDEX "chunk_type_scope_id_idx" ON "chunk_type" USING btree ("id");--> statement-breakpoint
CREATE INDEX "chunk_type_userId_idx" ON "chunk_type" USING btree ("user_id");--> statement-breakpoint
CREATE INDEX "chunk_type_codebaseId_idx" ON "chunk_type" USING btree ("codebase_id");--> statement-breakpoint
CREATE INDEX "chunk_version_chunkId_idx" ON "chunk_version" USING btree ("chunk_id");--> statement-breakpoint
CREATE INDEX "chunk_userId_idx" ON "chunk" USING btree ("user_id");--> statement-breakpoint
CREATE INDEX "chunk_type_idx" ON "chunk" USING btree ("type");--> statement-breakpoint
CREATE INDEX "chunk_archivedAt_idx" ON "chunk" USING btree ("archived_at");--> statement-breakpoint
CREATE INDEX "chunk_updatedAt_idx" ON "chunk" USING btree ("updated_at");--> statement-breakpoint
CREATE UNIQUE INDEX "chunk_document_order_idx" ON "chunk" USING btree ("document_id","document_order") WHERE "chunk"."document_id" IS NOT NULL;--> statement-breakpoint
CREATE INDEX "connection_sourceId_idx" ON "chunk_connection" USING btree ("source_id");--> statement-breakpoint
CREATE INDEX "connection_targetId_idx" ON "chunk_connection" USING btree ("target_id");--> statement-breakpoint
CREATE UNIQUE INDEX "connection_unique_idx" ON "chunk_connection" USING btree ("source_id","target_id","relation");--> statement-breakpoint
CREATE INDEX "chunk_codebase_chunkId_idx" ON "chunk_codebase" USING btree ("chunk_id");--> statement-breakpoint
CREATE INDEX "chunk_codebase_codebaseId_idx" ON "chunk_codebase" USING btree ("codebase_id");--> statement-breakpoint
CREATE UNIQUE INDEX "codebase_user_name_idx" ON "codebase" USING btree ("user_id","name");--> statement-breakpoint
CREATE UNIQUE INDEX "codebase_user_remote_idx" ON "codebase" USING btree ("user_id","remote_url") WHERE "remote_url" IS NOT NULL;--> statement-breakpoint
CREATE INDEX "codebase_userId_idx" ON "codebase" USING btree ("user_id");--> statement-breakpoint
CREATE INDEX "collection_userId_idx" ON "collection" USING btree ("user_id");--> statement-breakpoint
CREATE UNIQUE INDEX "collection_user_name_idx" ON "collection" USING btree ("user_id","name");--> statement-breakpoint
CREATE INDEX "chunk_comment_chunkId_idx" ON "chunk_comment" USING btree ("chunk_id");--> statement-breakpoint
CREATE INDEX "chunk_comment_userId_idx" ON "chunk_comment" USING btree ("user_id");--> statement-breakpoint
CREATE INDEX "connection_relation_userId_idx" ON "connection_relation" USING btree ("user_id");--> statement-breakpoint
CREATE INDEX "connection_relation_codebaseId_idx" ON "connection_relation" USING btree ("codebase_id");--> statement-breakpoint
CREATE UNIQUE INDEX "document_source_codebase_user_idx" ON "document" USING btree ("source_path","codebase_id","user_id");--> statement-breakpoint
CREATE INDEX "document_userId_idx" ON "document" USING btree ("user_id");--> statement-breakpoint
CREATE INDEX "document_codebaseId_idx" ON "document" USING btree ("codebase_id");--> statement-breakpoint
CREATE UNIQUE INDEX "favorite_user_chunk_idx" ON "user_favorite" USING btree ("user_id","chunk_id");--> statement-breakpoint
CREATE INDEX "favorite_userId_idx" ON "user_favorite" USING btree ("user_id");--> statement-breakpoint
CREATE UNIQUE INDEX "chunk_feature_delta_chunk_feature_idx" ON "chunk_feature_delta" USING btree ("chunk_id","feature_id");--> statement-breakpoint
CREATE INDEX "chunk_feature_delta_feature_idx" ON "chunk_feature_delta" USING btree ("feature_id");--> statement-breakpoint
CREATE UNIQUE INDEX "feature_user_name_idx" ON "feature" USING btree ("user_id","name");--> statement-breakpoint
CREATE UNIQUE INDEX "feature_user_priority_idx" ON "feature" USING btree ("user_id","priority");--> statement-breakpoint
CREATE INDEX "chunk_file_ref_chunkId_idx" ON "chunk_file_ref" USING btree ("chunk_id");--> statement-breakpoint
CREATE INDEX "chunk_file_ref_path_idx" ON "chunk_file_ref" USING btree ("path");--> statement-breakpoint
CREATE INDEX "chunk_tag_tagId_idx" ON "chunk_tag" USING btree ("tag_id");--> statement-breakpoint
CREATE UNIQUE INDEX "tag_user_name_idx" ON "tag" USING btree ("user_id","name");--> statement-breakpoint
CREATE UNIQUE INDEX "tag_type_user_name_idx" ON "tag_type" USING btree ("user_id","name");--> statement-breakpoint
CREATE UNIQUE INDEX "template_user_name_idx" ON "chunk_template" USING btree ("user_id","name") WHERE "user_id" IS NOT NULL;--> statement-breakpoint
CREATE UNIQUE INDEX "template_builtin_name_idx" ON "chunk_template" USING btree ("name") WHERE "user_id" IS NULL;--> statement-breakpoint
CREATE INDEX "use_case_userId_idx" ON "use_case" USING btree ("user_id");--> statement-breakpoint
CREATE UNIQUE INDEX "use_case_user_name_idx" ON "use_case" USING btree ("user_id","name");--> statement-breakpoint
CREATE INDEX "use_case_parentId_idx" ON "use_case" USING btree ("parent_id");--> statement-breakpoint
CREATE INDEX "requirement_userId_idx" ON "requirement" USING btree ("user_id");--> statement-breakpoint
CREATE INDEX "requirement_codebaseId_idx" ON "requirement" USING btree ("codebase_id");--> statement-breakpoint
CREATE INDEX "requirement_status_idx" ON "requirement" USING btree ("status");--> statement-breakpoint
CREATE UNIQUE INDEX "vocabulary_codebase_word_cat_idx" ON "vocabulary_entry" USING btree ("codebase_id","category",lower("word"));--> statement-breakpoint
CREATE INDEX "vocabulary_codebaseId_idx" ON "vocabulary_entry" USING btree ("codebase_id");--> statement-breakpoint
CREATE INDEX "notification_userId_idx" ON "notification" USING btree ("user_id");--> statement-breakpoint
CREATE INDEX "notification_userId_read_idx" ON "notification" USING btree ("user_id","read");--> statement-breakpoint
CREATE UNIQUE INDEX "codebase_settings_cb_key_idx" ON "codebase_settings" USING btree ("codebase_id","key");--> statement-breakpoint
CREATE INDEX "codebase_settings_codebaseId_idx" ON "codebase_settings" USING btree ("codebase_id");--> statement-breakpoint
CREATE UNIQUE INDEX "user_settings_user_key_idx" ON "user_settings" USING btree ("user_id","key");--> statement-breakpoint
CREATE INDEX "user_settings_userId_idx" ON "user_settings" USING btree ("user_id");--> statement-breakpoint
CREATE INDEX "plan_userId_idx" ON "plan" USING btree ("user_id");--> statement-breakpoint
CREATE INDEX "plan_codebaseId_idx" ON "plan" USING btree ("codebase_id");--> statement-breakpoint
CREATE INDEX "plan_analyze_item_planId_idx" ON "plan_analyze_item" USING btree ("plan_id");--> statement-breakpoint
CREATE INDEX "plan_analyze_item_chunkId_idx" ON "plan_analyze_item" USING btree ("chunk_id");--> statement-breakpoint
CREATE INDEX "plan_external_link_planId_idx" ON "plan_external_link" USING btree ("plan_id");--> statement-breakpoint
CREATE UNIQUE INDEX "plan_requirement_unique_idx" ON "plan_requirement" USING btree ("plan_id","requirement_id");--> statement-breakpoint
CREATE INDEX "plan_requirement_planId_idx" ON "plan_requirement" USING btree ("plan_id");--> statement-breakpoint
CREATE INDEX "plan_task_planId_idx" ON "plan_task" USING btree ("plan_id");--> statement-breakpoint
CREATE UNIQUE INDEX "plan_task_chunk_unique_idx" ON "plan_task_chunk" USING btree ("task_id","chunk_id","relation");--> statement-breakpoint
CREATE INDEX "plan_task_chunk_taskId_idx" ON "plan_task_chunk" USING btree ("task_id");--> statement-breakpoint
CREATE UNIQUE INDEX "plan_task_dependency_unique_idx" ON "plan_task_dependency" USING btree ("task_id","depends_on_task_id");--> statement-breakpoint
CREATE INDEX "plan_task_dependency_taskId_idx" ON "plan_task_dependency" USING btree ("task_id");--> statement-breakpoint
CREATE INDEX "plan_task_external_link_taskId_idx" ON "plan_task_external_link" USING btree ("task_id");--> statement-breakpoint
CREATE UNIQUE INDEX "workspace_user_name_idx" ON "workspace" USING btree ("user_id","name");--> statement-breakpoint
CREATE INDEX "workspace_userId_idx" ON "workspace" USING btree ("user_id");--> statement-breakpoint
CREATE INDEX "workspace_codebase_workspaceId_idx" ON "workspace_codebase" USING btree ("workspace_id");--> statement-breakpoint
CREATE INDEX "workspace_codebase_codebaseId_idx" ON "workspace_codebase" USING btree ("codebase_id");--> statement-breakpoint
CREATE INDEX "saved_graph_userId_idx" ON "saved_graph" USING btree ("user_id");--> statement-breakpoint
CREATE INDEX "saved_query_userId_idx" ON "saved_query" USING btree ("user_id");--> statement-breakpoint
CREATE INDEX "chunk_staleness_chunkId_idx" ON "chunk_staleness" USING btree ("chunk_id");--> statement-breakpoint
CREATE INDEX "chunk_staleness_reason_idx" ON "chunk_staleness" USING btree ("reason");--> statement-breakpoint
CREATE INDEX "chunk_staleness_dismissedAt_idx" ON "chunk_staleness" USING btree ("dismissed_at");--> statement-breakpoint
CREATE INDEX "staleness_scan_codebaseId_idx" ON "staleness_scan" USING btree ("codebase_id");--> statement-breakpoint
CREATE INDEX "learning_path_userId_idx" ON "learning_path" USING btree ("user_id");