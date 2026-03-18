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
	"origin" text DEFAULT 'human' NOT NULL,
	"review_status" text DEFAULT 'approved' NOT NULL,
	"reviewed_by" text,
	"reviewed_at" timestamp,
	"archived_at" timestamp
);
--> statement-breakpoint
CREATE TABLE "chunk_connection" (
	"id" text PRIMARY KEY NOT NULL,
	"source_id" text NOT NULL,
	"target_id" text NOT NULL,
	"relation" text DEFAULT 'related' NOT NULL,
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
CREATE TABLE "user_favorite" (
	"id" text PRIMARY KEY NOT NULL,
	"user_id" text NOT NULL,
	"chunk_id" text NOT NULL,
	"order" integer DEFAULT 0 NOT NULL,
	"created_at" timestamp DEFAULT now() NOT NULL
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
	"created_at" timestamp DEFAULT now() NOT NULL,
	"updated_at" timestamp DEFAULT now() NOT NULL
);
--> statement-breakpoint
CREATE TABLE "requirement" (
	"id" text PRIMARY KEY NOT NULL,
	"title" text NOT NULL,
	"description" text,
	"steps" jsonb NOT NULL,
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
ALTER TABLE "activity_log" ADD CONSTRAINT "activity_log_user_id_user_id_fk" FOREIGN KEY ("user_id") REFERENCES "public"."user"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "activity_log" ADD CONSTRAINT "activity_log_codebase_id_codebase_id_fk" FOREIGN KEY ("codebase_id") REFERENCES "public"."codebase"("id") ON DELETE set null ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "chunk_applies_to" ADD CONSTRAINT "chunk_applies_to_chunk_id_chunk_id_fk" FOREIGN KEY ("chunk_id") REFERENCES "public"."chunk"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "account" ADD CONSTRAINT "account_user_id_user_id_fk" FOREIGN KEY ("user_id") REFERENCES "public"."user"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "session" ADD CONSTRAINT "session_user_id_user_id_fk" FOREIGN KEY ("user_id") REFERENCES "public"."user"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "chunk_version" ADD CONSTRAINT "chunk_version_chunk_id_chunk_id_fk" FOREIGN KEY ("chunk_id") REFERENCES "public"."chunk"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "chunk" ADD CONSTRAINT "chunk_user_id_user_id_fk" FOREIGN KEY ("user_id") REFERENCES "public"."user"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "chunk" ADD CONSTRAINT "chunk_reviewed_by_user_id_fk" FOREIGN KEY ("reviewed_by") REFERENCES "public"."user"("id") ON DELETE set null ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "chunk_connection" ADD CONSTRAINT "chunk_connection_source_id_chunk_id_fk" FOREIGN KEY ("source_id") REFERENCES "public"."chunk"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "chunk_connection" ADD CONSTRAINT "chunk_connection_target_id_chunk_id_fk" FOREIGN KEY ("target_id") REFERENCES "public"."chunk"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "chunk_connection" ADD CONSTRAINT "chunk_connection_reviewed_by_user_id_fk" FOREIGN KEY ("reviewed_by") REFERENCES "public"."user"("id") ON DELETE set null ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "chunk_codebase" ADD CONSTRAINT "chunk_codebase_chunk_id_chunk_id_fk" FOREIGN KEY ("chunk_id") REFERENCES "public"."chunk"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "chunk_codebase" ADD CONSTRAINT "chunk_codebase_codebase_id_codebase_id_fk" FOREIGN KEY ("codebase_id") REFERENCES "public"."codebase"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "codebase" ADD CONSTRAINT "codebase_user_id_user_id_fk" FOREIGN KEY ("user_id") REFERENCES "public"."user"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "collection" ADD CONSTRAINT "collection_user_id_user_id_fk" FOREIGN KEY ("user_id") REFERENCES "public"."user"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "collection" ADD CONSTRAINT "collection_codebase_id_codebase_id_fk" FOREIGN KEY ("codebase_id") REFERENCES "public"."codebase"("id") ON DELETE set null ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "chunk_comment" ADD CONSTRAINT "chunk_comment_chunk_id_chunk_id_fk" FOREIGN KEY ("chunk_id") REFERENCES "public"."chunk"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "chunk_comment" ADD CONSTRAINT "chunk_comment_user_id_user_id_fk" FOREIGN KEY ("user_id") REFERENCES "public"."user"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "user_favorite" ADD CONSTRAINT "user_favorite_user_id_user_id_fk" FOREIGN KEY ("user_id") REFERENCES "public"."user"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "user_favorite" ADD CONSTRAINT "user_favorite_chunk_id_chunk_id_fk" FOREIGN KEY ("chunk_id") REFERENCES "public"."chunk"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
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
ALTER TABLE "requirement" ADD CONSTRAINT "requirement_codebase_id_codebase_id_fk" FOREIGN KEY ("codebase_id") REFERENCES "public"."codebase"("id") ON DELETE set null ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "requirement" ADD CONSTRAINT "requirement_user_id_user_id_fk" FOREIGN KEY ("user_id") REFERENCES "public"."user"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "requirement" ADD CONSTRAINT "requirement_use_case_id_use_case_id_fk" FOREIGN KEY ("use_case_id") REFERENCES "public"."use_case"("id") ON DELETE set null ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "requirement" ADD CONSTRAINT "requirement_reviewed_by_user_id_fk" FOREIGN KEY ("reviewed_by") REFERENCES "public"."user"("id") ON DELETE set null ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "requirement_chunk" ADD CONSTRAINT "requirement_chunk_requirement_id_requirement_id_fk" FOREIGN KEY ("requirement_id") REFERENCES "public"."requirement"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "requirement_chunk" ADD CONSTRAINT "requirement_chunk_chunk_id_chunk_id_fk" FOREIGN KEY ("chunk_id") REFERENCES "public"."chunk"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "vocabulary_entry" ADD CONSTRAINT "vocabulary_entry_codebase_id_codebase_id_fk" FOREIGN KEY ("codebase_id") REFERENCES "public"."codebase"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "vocabulary_entry" ADD CONSTRAINT "vocabulary_entry_user_id_user_id_fk" FOREIGN KEY ("user_id") REFERENCES "public"."user"("id") ON DELETE set null ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "notification" ADD CONSTRAINT "notification_user_id_user_id_fk" FOREIGN KEY ("user_id") REFERENCES "public"."user"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "codebase_settings" ADD CONSTRAINT "codebase_settings_codebase_id_codebase_id_fk" FOREIGN KEY ("codebase_id") REFERENCES "public"."codebase"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "user_settings" ADD CONSTRAINT "user_settings_user_id_user_id_fk" FOREIGN KEY ("user_id") REFERENCES "public"."user"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
CREATE INDEX "activity_userId_idx" ON "activity_log" USING btree ("user_id");--> statement-breakpoint
CREATE INDEX "activity_createdAt_idx" ON "activity_log" USING btree ("created_at");--> statement-breakpoint
CREATE INDEX "chunk_applies_to_chunkId_idx" ON "chunk_applies_to" USING btree ("chunk_id");--> statement-breakpoint
CREATE INDEX "account_userId_idx" ON "account" USING btree ("user_id");--> statement-breakpoint
CREATE INDEX "session_userId_idx" ON "session" USING btree ("user_id");--> statement-breakpoint
CREATE INDEX "verification_identifier_idx" ON "verification" USING btree ("identifier");--> statement-breakpoint
CREATE INDEX "chunk_version_chunkId_idx" ON "chunk_version" USING btree ("chunk_id");--> statement-breakpoint
CREATE INDEX "chunk_userId_idx" ON "chunk" USING btree ("user_id");--> statement-breakpoint
CREATE INDEX "chunk_type_idx" ON "chunk" USING btree ("type");--> statement-breakpoint
CREATE INDEX "connection_sourceId_idx" ON "chunk_connection" USING btree ("source_id");--> statement-breakpoint
CREATE INDEX "connection_targetId_idx" ON "chunk_connection" USING btree ("target_id");--> statement-breakpoint
CREATE UNIQUE INDEX "connection_unique_idx" ON "chunk_connection" USING btree ("source_id","target_id","relation");--> statement-breakpoint
CREATE INDEX "chunk_codebase_chunkId_idx" ON "chunk_codebase" USING btree ("chunk_id");--> statement-breakpoint
CREATE UNIQUE INDEX "codebase_user_name_idx" ON "codebase" USING btree ("user_id","name");--> statement-breakpoint
CREATE UNIQUE INDEX "codebase_user_remote_idx" ON "codebase" USING btree ("user_id","remote_url") WHERE "remote_url" IS NOT NULL;--> statement-breakpoint
CREATE INDEX "codebase_userId_idx" ON "codebase" USING btree ("user_id");--> statement-breakpoint
CREATE INDEX "collection_userId_idx" ON "collection" USING btree ("user_id");--> statement-breakpoint
CREATE UNIQUE INDEX "collection_user_name_idx" ON "collection" USING btree ("user_id","name");--> statement-breakpoint
CREATE INDEX "chunk_comment_chunkId_idx" ON "chunk_comment" USING btree ("chunk_id");--> statement-breakpoint
CREATE UNIQUE INDEX "favorite_user_chunk_idx" ON "user_favorite" USING btree ("user_id","chunk_id");--> statement-breakpoint
CREATE INDEX "favorite_userId_idx" ON "user_favorite" USING btree ("user_id");--> statement-breakpoint
CREATE INDEX "chunk_file_ref_chunkId_idx" ON "chunk_file_ref" USING btree ("chunk_id");--> statement-breakpoint
CREATE INDEX "chunk_file_ref_path_idx" ON "chunk_file_ref" USING btree ("path");--> statement-breakpoint
CREATE UNIQUE INDEX "tag_user_name_idx" ON "tag" USING btree ("user_id","name");--> statement-breakpoint
CREATE UNIQUE INDEX "tag_type_user_name_idx" ON "tag_type" USING btree ("user_id","name");--> statement-breakpoint
CREATE UNIQUE INDEX "template_user_name_idx" ON "chunk_template" USING btree ("user_id","name") WHERE "user_id" IS NOT NULL;--> statement-breakpoint
CREATE UNIQUE INDEX "template_builtin_name_idx" ON "chunk_template" USING btree ("name") WHERE "user_id" IS NULL;--> statement-breakpoint
CREATE INDEX "use_case_userId_idx" ON "use_case" USING btree ("user_id");--> statement-breakpoint
CREATE UNIQUE INDEX "use_case_user_name_idx" ON "use_case" USING btree ("user_id","name");--> statement-breakpoint
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
CREATE INDEX "user_settings_userId_idx" ON "user_settings" USING btree ("user_id");