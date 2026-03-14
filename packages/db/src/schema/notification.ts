import { boolean, index, pgTable, text, timestamp } from "drizzle-orm/pg-core";

import { user } from "./auth";

export const notification = pgTable(
    "notification",
    {
        id: text("id").primaryKey(),
        userId: text("user_id")
            .notNull()
            .references(() => user.id, { onDelete: "cascade" }),
        type: text("type").notNull(), // "stale_chunks", "review_needed", "ai_suggestion", "chunk_updated"
        title: text("title").notNull(),
        message: text("message").notNull(),
        linkTo: text("link_to"), // URL path like "/chunks/abc-123"
        read: boolean("read").notNull().default(false),
        createdAt: timestamp("created_at").defaultNow().notNull()
    },
    table => [
        index("notification_userId_idx").on(table.userId),
        index("notification_userId_read_idx").on(table.userId, table.read)
    ]
);
