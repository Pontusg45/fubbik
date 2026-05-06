import { pgTable, text, timestamp, integer, jsonb, index } from "drizzle-orm/pg-core";

import { chunk } from "./chunk";

export const chunkVersion = pgTable(
    "chunk_version",
    {
        id: text("id").primaryKey(),
        chunkId: text("chunk_id")
            .notNull()
            .references(() => chunk.id, { onDelete: "cascade" }),
        version: integer("version").notNull(),
        title: text("title").notNull(),
        content: text("content").notNull(),
        type: text("type").notNull(),
        tags: jsonb("tags").$type<string[]>().notNull(),
        rationale: text("rationale"),
        alternatives: jsonb("alternatives").$type<string[]>(),
        consequences: text("consequences"),
        scope: jsonb("scope").$type<Record<string, string>>(),
        updateTag: text("update_tag"),
        createdAt: timestamp("created_at").defaultNow().notNull()
    },
    table => [
        index("chunk_version_chunkId_idx").on(table.chunkId),
        index("chunk_version_update_tag_idx").on(table.updateTag)
    ]
);
