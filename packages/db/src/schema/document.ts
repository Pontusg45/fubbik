import { relations } from "drizzle-orm";
import { index, integer, pgTable, text, timestamp, uniqueIndex } from "drizzle-orm/pg-core";

import { chunk } from "./chunk";
import { space } from "./space";
import { user } from "./auth";

export const document = pgTable(
    "document",
    {
        id: text("id").primaryKey(),
        title: text("title").notNull(),
        sourcePath: text("source_path").notNull(),
        contentHash: text("content_hash").notNull(),
        description: text("description"),
        splitLevel: integer("split_level"),
        spaceId: text("space_id").references(() => space.id, { onDelete: "set null" }),
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
        uniqueIndex("document_source_space_user_idx").on(table.sourcePath, table.spaceId, table.userId),
        index("document_userId_idx").on(table.userId),
        index("document_spaceId_idx").on(table.spaceId)
    ]
);

export const documentRelations = relations(document, ({ one, many }) => ({
    user: one(user, { fields: [document.userId], references: [user.id] }),
    space: one(space, { fields: [document.spaceId], references: [space.id] }),
    chunks: many(chunk)
}));
