import { relations } from "drizzle-orm";
import { index, jsonb, pgTable, text, timestamp } from "drizzle-orm/pg-core";

import { user } from "./auth";
import { space } from "./space";

export const savedQuery = pgTable(
    "saved_query",
    {
        id: text("id").primaryKey(),
        name: text("name").notNull(),
        query: jsonb("query").notNull(),
        userId: text("user_id")
            .notNull()
            .references(() => user.id, { onDelete: "cascade" }),
        spaceId: text("space_id").references(() => space.id, { onDelete: "set null" }),
        createdAt: timestamp("created_at").defaultNow().notNull()
    },
    table => [
        index("saved_query_userId_idx").on(table.userId)
    ]
);

export const savedQueryRelations = relations(savedQuery, ({ one }) => ({
    user: one(user, { fields: [savedQuery.userId], references: [user.id] }),
    space: one(space, { fields: [savedQuery.spaceId], references: [space.id] })
}));
