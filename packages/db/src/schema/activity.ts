import { index, pgTable, text, timestamp } from "drizzle-orm/pg-core";

import { user } from "./auth";
import { codebase } from "./codebase";

export const activityLog = pgTable(
    "activity_log",
    {
        id: text("id").primaryKey(),
        userId: text("user_id")
            .notNull()
            .references(() => user.id, { onDelete: "cascade" }),
        entityType: text("entity_type").notNull(), // "chunk", "requirement", "connection", "tag", "codebase"
        entityId: text("entity_id").notNull(),
        entityTitle: text("entity_title"),
        action: text("action").notNull(), // "created", "updated", "deleted", "archived", "restored"
        codebaseId: text("codebase_id").references(() => codebase.id, { onDelete: "set null" }),
        createdAt: timestamp("created_at").defaultNow().notNull()
    },
    table => [
        index("activity_userId_idx").on(table.userId),
        index("activity_createdAt_idx").on(table.createdAt),
        index("activity_codebaseId_idx").on(table.codebaseId)
    ]
);
