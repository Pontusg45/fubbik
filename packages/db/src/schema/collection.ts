import { index, jsonb, pgTable, text, timestamp, uniqueIndex } from "drizzle-orm/pg-core";

import { user } from "./auth";
import { space } from "./space";

export interface CollectionFilter {
    type?: string;
    tags?: string;
    search?: string;
    sort?: string;
    after?: string;
    enrichment?: string;
    minConnections?: string;
    origin?: string;
    reviewStatus?: string;
}

export const collection = pgTable(
    "collection",
    {
        id: text("id").primaryKey(),
        name: text("name").notNull(),
        description: text("description"),
        filter: jsonb("filter").$type<CollectionFilter>().notNull(),
        userId: text("user_id")
            .notNull()
            .references(() => user.id, { onDelete: "cascade" }),
        spaceId: text("space_id").references(() => space.id, { onDelete: "set null" }),
        createdAt: timestamp("created_at").defaultNow().notNull(),
        updatedAt: timestamp("updated_at")
            .defaultNow()
            .$onUpdate(() => new Date())
            .notNull()
    },
    table => [
        index("collection_userId_idx").on(table.userId),
        uniqueIndex("collection_user_name_idx").on(table.userId, table.name)
    ]
);
