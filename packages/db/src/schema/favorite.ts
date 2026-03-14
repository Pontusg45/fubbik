import { index, integer, pgTable, text, timestamp, uniqueIndex } from "drizzle-orm/pg-core";

import { user } from "./auth";
import { chunk } from "./chunk";

export const userFavorite = pgTable(
    "user_favorite",
    {
        id: text("id").primaryKey(),
        userId: text("user_id")
            .notNull()
            .references(() => user.id, { onDelete: "cascade" }),
        chunkId: text("chunk_id")
            .notNull()
            .references(() => chunk.id, { onDelete: "cascade" }),
        order: integer("order").notNull().default(0),
        createdAt: timestamp("created_at").defaultNow().notNull()
    },
    table => [
        uniqueIndex("favorite_user_chunk_idx").on(table.userId, table.chunkId),
        index("favorite_userId_idx").on(table.userId)
    ]
);
