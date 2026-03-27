import { relations } from "drizzle-orm";
import { index, jsonb, pgTable, text, timestamp } from "drizzle-orm/pg-core";

import { user } from "./auth";
import { codebase } from "./codebase";

export const savedGraph = pgTable(
    "saved_graph",
    {
        id: text("id").primaryKey(),
        name: text("name").notNull(),
        description: text("description"),
        // Which chunks to include
        chunkIds: jsonb("chunk_ids").$type<string[]>().notNull().default([]),
        // Node positions: { [chunkId]: { x: number, y: number } }
        positions: jsonb("positions")
            .$type<Record<string, { x: number; y: number }>>()
            .notNull()
            .default({}),
        // Layout settings
        layoutAlgorithm: text("layout_algorithm").notNull().default("force"),
        // Ownership
        userId: text("user_id")
            .notNull()
            .references(() => user.id, { onDelete: "cascade" }),
        codebaseId: text("codebase_id").references(() => codebase.id, { onDelete: "set null" }),
        createdAt: timestamp("created_at").defaultNow().notNull(),
        updatedAt: timestamp("updated_at")
            .defaultNow()
            .$onUpdate(() => new Date())
            .notNull()
    },
    table => [index("saved_graph_userId_idx").on(table.userId)]
);

export const savedGraphRelations = relations(savedGraph, ({ one }) => ({
    user: one(user, { fields: [savedGraph.userId], references: [user.id] }),
    codebase: one(codebase, { fields: [savedGraph.codebaseId], references: [codebase.id] })
}));
