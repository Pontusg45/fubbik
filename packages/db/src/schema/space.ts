import { relations } from "drizzle-orm";
import { index, pgTable, primaryKey, text, timestamp, uniqueIndex } from "drizzle-orm/pg-core";

import { user } from "./auth";
import { chunk } from "./chunk";
import { spaceKind } from "./space-kind";

export const space = pgTable(
    "space",
    {
        id: text("id").primaryKey(),
        name: text("name").notNull(),
        kind: text("kind").notNull().references(() => spaceKind.id, { onDelete: "restrict" }),
        description: text("description"),
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
        uniqueIndex("space_user_name_idx").on(table.userId, table.name),
        index("space_userId_idx").on(table.userId),
        index("space_kind_idx").on(table.kind)
    ]
);

export const chunkSpace = pgTable(
    "chunk_space",
    {
        chunkId: text("chunk_id")
            .notNull()
            .references(() => chunk.id, { onDelete: "cascade" }),
        spaceId: text("space_id")
            .notNull()
            .references(() => space.id, { onDelete: "cascade" })
    },
    table => [
        primaryKey({ columns: [table.chunkId, table.spaceId] }),
        index("chunk_space_chunkId_idx").on(table.chunkId),
        index("chunk_space_spaceId_idx").on(table.spaceId)
    ]
);

export const spaceRelations = relations(space, ({ one, many }) => ({
    user: one(user, { fields: [space.userId], references: [user.id] }),
    spaceKind: one(spaceKind, { fields: [space.kind], references: [spaceKind.id] }),
    chunkSpaces: many(chunkSpace)
}));

export const chunkSpaceRelations = relations(chunkSpace, ({ one }) => ({
    chunk: one(chunk, { fields: [chunkSpace.chunkId], references: [chunk.id] }),
    space: one(space, { fields: [chunkSpace.spaceId], references: [space.id] })
}));
