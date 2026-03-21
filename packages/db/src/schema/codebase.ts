import { relations, sql } from "drizzle-orm";
import { index, jsonb, pgTable, primaryKey, text, timestamp, uniqueIndex } from "drizzle-orm/pg-core";

import { user } from "./auth";
import { chunk } from "./chunk";

export const codebase = pgTable(
    "codebase",
    {
        id: text("id").primaryKey(),
        name: text("name").notNull(),
        remoteUrl: text("remote_url"),
        localPaths: jsonb("local_paths").$type<string[]>().notNull().default([]),
        userId: text("user_id")
            .notNull()
            .references(() => user.id, { onDelete: "cascade" }),
        createdAt: timestamp("created_at").defaultNow().notNull(),
        updatedAt: timestamp("updated_at")
            .defaultNow()
            .$onUpdate(() => new Date())
            .notNull()
    },
    table => [
        uniqueIndex("codebase_user_name_idx").on(table.userId, table.name),
        uniqueIndex("codebase_user_remote_idx")
            .on(table.userId, table.remoteUrl)
            .where(sql`"remote_url" IS NOT NULL`),
        index("codebase_userId_idx").on(table.userId)
    ]
);

export const chunkCodebase = pgTable(
    "chunk_codebase",
    {
        chunkId: text("chunk_id")
            .notNull()
            .references(() => chunk.id, { onDelete: "cascade" }),
        codebaseId: text("codebase_id")
            .notNull()
            .references(() => codebase.id, { onDelete: "cascade" })
    },
    table => [
        primaryKey({ columns: [table.chunkId, table.codebaseId] }),
        index("chunk_codebase_chunkId_idx").on(table.chunkId),
        index("chunk_codebase_codebaseId_idx").on(table.codebaseId)
    ]
);

export const codebaseRelations = relations(codebase, ({ one, many }) => ({
    user: one(user, { fields: [codebase.userId], references: [user.id] }),
    chunkCodebases: many(chunkCodebase)
}));

export const chunkCodebaseRelations = relations(chunkCodebase, ({ one }) => ({
    chunk: one(chunk, { fields: [chunkCodebase.chunkId], references: [chunk.id] }),
    codebase: one(codebase, { fields: [chunkCodebase.codebaseId], references: [codebase.id] })
}));
