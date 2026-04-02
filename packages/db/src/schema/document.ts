import { relations } from "drizzle-orm";
import { index, pgTable, text, timestamp, uniqueIndex } from "drizzle-orm/pg-core";

import { chunk } from "./chunk";
import { codebase } from "./codebase";
import { user } from "./auth";

export const document = pgTable(
    "document",
    {
        id: text("id").primaryKey(),
        title: text("title").notNull(),
        sourcePath: text("source_path").notNull(),
        contentHash: text("content_hash").notNull(),
        description: text("description"),
        codebaseId: text("codebase_id").references(() => codebase.id, { onDelete: "set null" }),
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
        uniqueIndex("document_source_codebase_user_idx").on(table.sourcePath, table.codebaseId, table.userId),
        index("document_userId_idx").on(table.userId),
        index("document_codebaseId_idx").on(table.codebaseId)
    ]
);

export const documentRelations = relations(document, ({ one, many }) => ({
    user: one(user, { fields: [document.userId], references: [user.id] }),
    codebase: one(codebase, { fields: [document.codebaseId], references: [codebase.id] }),
    chunks: many(chunk)
}));
