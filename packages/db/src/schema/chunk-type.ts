import { relations } from "drizzle-orm";
import { boolean, index, integer, jsonb, pgTable, text, timestamp, uniqueIndex } from "drizzle-orm/pg-core";

import { user } from "./auth";
import { codebase } from "./codebase";

/**
 * Catalog of chunk types.
 *
 * Builtin rows are seeded (userId = null, codebaseId = null, builtIn = true).
 * Users / codebases can add custom types scoped by userId and/or codebaseId.
 * `chunk.type` is still a loose text column today; FK cutover happens in a later step.
 */
export const chunkType = pgTable(
    "chunk_type",
    {
        id: text("id").primaryKey(), // slug — e.g. "document", "convention", "runbook"
        label: text("label").notNull(),
        description: text("description"),
        icon: text("icon"), // lucide icon name (e.g., "FileText")
        color: text("color").notNull().default("#8b5cf6"),
        examples: jsonb("examples").$type<string[]>().notNull().default([]),
        displayOrder: integer("display_order").notNull().default(100),
        builtIn: boolean("built_in").notNull().default(false),
        userId: text("user_id").references(() => user.id, { onDelete: "cascade" }),
        codebaseId: text("codebase_id").references(() => codebase.id, { onDelete: "cascade" }),
        createdAt: timestamp("created_at").defaultNow().notNull(),
        updatedAt: timestamp("updated_at")
            .defaultNow()
            .$onUpdate(() => new Date())
            .notNull()
    },
    table => [
        uniqueIndex("chunk_type_scope_id_idx").on(table.id),
        index("chunk_type_userId_idx").on(table.userId),
        index("chunk_type_codebaseId_idx").on(table.codebaseId)
    ]
);

export const chunkTypeRelations = relations(chunkType, ({ one }) => ({
    user: one(user, { fields: [chunkType.userId], references: [user.id] }),
    codebase: one(codebase, { fields: [chunkType.codebaseId], references: [codebase.id] })
}));
