import { relations } from "drizzle-orm";
import { index, pgTable, primaryKey, text, timestamp, uniqueIndex } from "drizzle-orm/pg-core";

import { user } from "./auth";
import { chunk } from "./chunk";

export const tagType = pgTable(
    "tag_type",
    {
        id: text("id").primaryKey(),
        name: text("name").notNull(),
        color: text("color").notNull().default("#8b5cf6"),
        icon: text("icon"),
        userId: text("user_id")
            .notNull()
            .references(() => user.id, { onDelete: "cascade" }),
        createdAt: timestamp("created_at").defaultNow().notNull()
    },
    table => [uniqueIndex("tag_type_user_name_idx").on(table.userId, table.name)]
);

export const tag = pgTable(
    "tag",
    {
        id: text("id").primaryKey(),
        name: text("name").notNull(),
        tagTypeId: text("tag_type_id").references(() => tagType.id, { onDelete: "set null" }),
        userId: text("user_id")
            .notNull()
            .references(() => user.id, { onDelete: "cascade" }),
        createdAt: timestamp("created_at").defaultNow().notNull(),
        origin: text("origin").notNull().default("human"),
        reviewStatus: text("review_status").notNull().default("approved"),
        reviewedBy: text("reviewed_by").references(() => user.id, { onDelete: "set null" }),
        reviewedAt: timestamp("reviewed_at")
    },
    table => [uniqueIndex("tag_user_name_idx").on(table.userId, table.name)]
);

export const chunkTag = pgTable(
    "chunk_tag",
    {
        chunkId: text("chunk_id")
            .notNull()
            .references(() => chunk.id, { onDelete: "cascade" }),
        tagId: text("tag_id")
            .notNull()
            .references(() => tag.id, { onDelete: "cascade" })
    },
    table => [
        primaryKey({ columns: [table.chunkId, table.tagId] }),
        index("chunk_tag_tagId_idx").on(table.tagId)
    ]
);

export const tagTypeRelations = relations(tagType, ({ one, many }) => ({
    user: one(user, { fields: [tagType.userId], references: [user.id] }),
    tags: many(tag)
}));

export const tagRelations = relations(tag, ({ one, many }) => ({
    user: one(user, { fields: [tag.userId], references: [user.id] }),
    tagType: one(tagType, { fields: [tag.tagTypeId], references: [tagType.id] }),
    chunkTags: many(chunkTag)
}));

export const chunkTagRelations = relations(chunkTag, ({ one }) => ({
    chunk: one(chunk, { fields: [chunkTag.chunkId], references: [chunk.id] }),
    tag: one(tag, { fields: [chunkTag.tagId], references: [tag.id] })
}));
