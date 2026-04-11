import { relations } from "drizzle-orm";
import { index, jsonb, pgTable, text, timestamp } from "drizzle-orm/pg-core";

import { user } from "./auth";

export const learningPath = pgTable(
    "learning_path",
    {
        id: text("id").primaryKey(),
        title: text("title").notNull(),
        description: text("description"),
        chunkIds: jsonb("chunk_ids").$type<string[]>().notNull().default([]),
        userId: text("user_id")
            .notNull()
            .references(() => user.id, { onDelete: "cascade" }),
        createdAt: timestamp("created_at").defaultNow().notNull(),
        updatedAt: timestamp("updated_at")
            .defaultNow()
            .$onUpdate(() => new Date())
            .notNull(),
    },
    table => [index("learning_path_userId_idx").on(table.userId)],
);

export const learningPathRelations = relations(learningPath, ({ one }) => ({
    user: one(user, { fields: [learningPath.userId], references: [user.id] }),
}));
