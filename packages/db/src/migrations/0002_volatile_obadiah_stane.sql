CREATE TABLE "implementation_session" (
	"id" text PRIMARY KEY NOT NULL,
	"title" text NOT NULL,
	"status" text DEFAULT 'in_progress' NOT NULL,
	"user_id" text NOT NULL,
	"codebase_id" text,
	"pr_url" text,
	"review_brief" text,
	"created_at" timestamp DEFAULT now() NOT NULL,
	"updated_at" timestamp DEFAULT now() NOT NULL,
	"completed_at" timestamp,
	"reviewed_at" timestamp
);
--> statement-breakpoint
CREATE TABLE "session_assumption" (
	"id" text PRIMARY KEY NOT NULL,
	"session_id" text NOT NULL,
	"description" text NOT NULL,
	"resolved" boolean DEFAULT false NOT NULL,
	"resolution" text
);
--> statement-breakpoint
CREATE TABLE "session_chunk_ref" (
	"session_id" text NOT NULL,
	"chunk_id" text NOT NULL,
	"reason" text NOT NULL,
	CONSTRAINT "session_chunk_ref_session_id_chunk_id_pk" PRIMARY KEY("session_id","chunk_id")
);
--> statement-breakpoint
CREATE TABLE "session_requirement_ref" (
	"session_id" text NOT NULL,
	"requirement_id" text NOT NULL,
	"steps_addressed" jsonb DEFAULT '[]'::jsonb NOT NULL,
	CONSTRAINT "session_requirement_ref_session_id_requirement_id_pk" PRIMARY KEY("session_id","requirement_id")
);
--> statement-breakpoint
ALTER TABLE "implementation_session" ADD CONSTRAINT "implementation_session_user_id_user_id_fk" FOREIGN KEY ("user_id") REFERENCES "public"."user"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "implementation_session" ADD CONSTRAINT "implementation_session_codebase_id_codebase_id_fk" FOREIGN KEY ("codebase_id") REFERENCES "public"."codebase"("id") ON DELETE set null ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "session_assumption" ADD CONSTRAINT "session_assumption_session_id_implementation_session_id_fk" FOREIGN KEY ("session_id") REFERENCES "public"."implementation_session"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "session_chunk_ref" ADD CONSTRAINT "session_chunk_ref_session_id_implementation_session_id_fk" FOREIGN KEY ("session_id") REFERENCES "public"."implementation_session"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "session_chunk_ref" ADD CONSTRAINT "session_chunk_ref_chunk_id_chunk_id_fk" FOREIGN KEY ("chunk_id") REFERENCES "public"."chunk"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "session_requirement_ref" ADD CONSTRAINT "session_requirement_ref_session_id_implementation_session_id_fk" FOREIGN KEY ("session_id") REFERENCES "public"."implementation_session"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "session_requirement_ref" ADD CONSTRAINT "session_requirement_ref_requirement_id_requirement_id_fk" FOREIGN KEY ("requirement_id") REFERENCES "public"."requirement"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
CREATE INDEX "impl_session_userId_idx" ON "implementation_session" USING btree ("user_id");--> statement-breakpoint
CREATE INDEX "impl_session_codebaseId_idx" ON "implementation_session" USING btree ("codebase_id");--> statement-breakpoint
CREATE INDEX "impl_session_status_idx" ON "implementation_session" USING btree ("status");--> statement-breakpoint
CREATE INDEX "assumption_sessionId_idx" ON "session_assumption" USING btree ("session_id");