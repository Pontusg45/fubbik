CREATE TABLE "requirement_dependency" (
	"requirement_id" text NOT NULL,
	"depends_on_id" text NOT NULL,
	CONSTRAINT "requirement_dependency_requirement_id_depends_on_id_pk" PRIMARY KEY("requirement_id","depends_on_id"),
	CONSTRAINT "no_self_dependency" CHECK ("requirement_dependency"."requirement_id" != "requirement_dependency"."depends_on_id")
);
--> statement-breakpoint
ALTER TABLE "use_case" ADD COLUMN "parent_id" text;--> statement-breakpoint
ALTER TABLE "requirement" ADD COLUMN "order" integer DEFAULT 0 NOT NULL;--> statement-breakpoint
ALTER TABLE "requirement_dependency" ADD CONSTRAINT "requirement_dependency_requirement_id_requirement_id_fk" FOREIGN KEY ("requirement_id") REFERENCES "public"."requirement"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "requirement_dependency" ADD CONSTRAINT "requirement_dependency_depends_on_id_requirement_id_fk" FOREIGN KEY ("depends_on_id") REFERENCES "public"."requirement"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "use_case" ADD CONSTRAINT "use_case_parent_id_use_case_id_fk" FOREIGN KEY ("parent_id") REFERENCES "public"."use_case"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
CREATE INDEX "use_case_parentId_idx" ON "use_case" USING btree ("parent_id");