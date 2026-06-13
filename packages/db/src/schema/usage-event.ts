import { relations } from "drizzle-orm";
import { index, jsonb, pgTable, text, timestamp } from "drizzle-orm/pg-core";

import { user } from "./auth";

export const usageEvent = pgTable(
    "usage_event",
    {
        id: text("id").primaryKey(),
        kind: text("kind").notNull(), // "context_query" | "chunk_view" | "mcp_resolve"
        chunkIds: jsonb("chunk_ids").$type<string[]>().notNull(),
        query: text("query"),
        userId: text("user_id")
            .notNull()
            .references(() => user.id, { onDelete: "cascade" }),
        createdAt: timestamp("created_at").defaultNow().notNull()
    },
    table => [
        index("usage_event_kind_idx").on(table.kind),
        index("usage_event_userId_idx").on(table.userId),
        index("usage_event_createdAt_idx").on(table.createdAt)
    ]
);

export const usageEventRelations = relations(usageEvent, ({ one }) => ({
    user: one(user, { fields: [usageEvent.userId], references: [user.id] })
}));

export type UsageEvent = typeof usageEvent.$inferSelect;
export type NewUsageEvent = typeof usageEvent.$inferInsert;
