import { relations, sql } from "drizzle-orm";
import { boolean, integer, jsonb, pgTable, text, timestamp, uniqueIndex } from "drizzle-orm/pg-core";

import { user } from "./auth";

export interface TemplateMatchRules {
    minScore: number;
    headings: Array<{
        patterns: string[];
        match: "exact" | "prefix" | "contains";
        level?: number;
        required: boolean;
    }>;
    frontmatter: Array<{
        key: string;
        match: "exact" | "oneOf" | "exists";
        value?: string;
        values?: string[];
    }>;
}

export interface TemplateFieldMapping {
    headings: string[];
    match: "exact" | "prefix" | "contains";
    target: "rationale" | "alternatives" | "consequences" | "summary" | "scope" | "content";
}

export const chunkTemplate = pgTable(
    "chunk_template",
    {
        id: text("id").primaryKey(),
        name: text("name").notNull(),
        description: text("description"),
        type: text("type").notNull().default("note"),
        content: text("content").notNull().default(""),
        isBuiltIn: boolean("is_built_in").notNull().default(false),
        matchRules: jsonb("match_rules").$type<TemplateMatchRules>(),
        fieldMappings: jsonb("field_mappings").$type<TemplateFieldMapping[]>(),
        priority: integer("priority").notNull().default(0),
        tags: text("tags").array(),
        userId: text("user_id").references(() => user.id, { onDelete: "cascade" }),
        createdAt: timestamp("created_at").defaultNow().notNull()
    },
    table => [
        uniqueIndex("template_user_name_idx")
            .on(table.userId, table.name)
            .where(sql`"user_id" IS NOT NULL`),
        uniqueIndex("template_builtin_name_idx")
            .on(table.name)
            .where(sql`"user_id" IS NULL`)
    ]
);

export const chunkTemplateRelations = relations(chunkTemplate, ({ one }) => ({
    user: one(user, { fields: [chunkTemplate.userId], references: [user.id] })
}));
