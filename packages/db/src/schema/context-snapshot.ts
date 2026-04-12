import { relations } from "drizzle-orm";
import { integer, jsonb, pgTable, text, timestamp } from "drizzle-orm/pg-core";

import { user } from "./auth";

export const contextSnapshot = pgTable("context_snapshot", {
    id: text("id").primaryKey().$defaultFn(() => crypto.randomUUID()),
    userId: text("user_id")
        .notNull()
        .references(() => user.id, { onDelete: "cascade" }),
    query: jsonb("query").notNull(),
    chunks: jsonb("chunks").notNull(),
    tokenCount: integer("token_count").notNull(),
    createdAt: timestamp("created_at").notNull().defaultNow(),
});

export const contextSnapshotRelations = relations(contextSnapshot, ({ one }) => ({
    user: one(user, { fields: [contextSnapshot.userId], references: [user.id] }),
}));

export type ContextSnapshot = typeof contextSnapshot.$inferSelect;
export type NewContextSnapshot = typeof contextSnapshot.$inferInsert;
