import { relations } from "drizzle-orm";
import { pgTable, text, timestamp, jsonb, index, uniqueIndex, customType } from "drizzle-orm/pg-core";

import { user } from "./auth";

const vector = customType<{ data: number[]; driverParam: string }>({
    dataType() {
        return "vector(768)";
    },
    toDriver(value: number[]) {
        return `[${value.join(",")}]`;
    },
    fromDriver(value: unknown) {
        return (value as string).slice(1, -1).split(",").map(Number);
    }
});

export const chunk = pgTable(
    "chunk",
    {
        id: text("id").primaryKey(),
        title: text("title").notNull(),
        content: text("content").notNull().default(""),
        type: text("type").notNull().default("note"),
        tags: jsonb("tags").$type<string[]>().notNull().default([]),
        userId: text("user_id")
            .notNull()
            .references(() => user.id, { onDelete: "cascade" }),
        createdAt: timestamp("created_at").defaultNow().notNull(),
        updatedAt: timestamp("updated_at")
            .defaultNow()
            .$onUpdate(() => new Date())
            .notNull(),
        summary: text("summary"),
        aliases: jsonb("aliases").$type<string[]>().notNull().default([]),
        notAbout: jsonb("not_about").$type<string[]>().notNull().default([]),
        scope: jsonb("scope").$type<Record<string, string>>().notNull().default({}),
        embedding: vector("embedding")
    },
    table => [index("chunk_userId_idx").on(table.userId), index("chunk_type_idx").on(table.type)]
);

export const chunkConnection = pgTable(
    "chunk_connection",
    {
        id: text("id").primaryKey(),
        sourceId: text("source_id")
            .notNull()
            .references(() => chunk.id, { onDelete: "cascade" }),
        targetId: text("target_id")
            .notNull()
            .references(() => chunk.id, { onDelete: "cascade" }),
        relation: text("relation").notNull().default("related"),
        createdAt: timestamp("created_at").defaultNow().notNull()
    },
    table => [
        index("connection_sourceId_idx").on(table.sourceId),
        index("connection_targetId_idx").on(table.targetId),
        uniqueIndex("connection_unique_idx").on(table.sourceId, table.targetId, table.relation)
    ]
);

export const chunkRelations = relations(chunk, ({ one, many }) => ({
    user: one(user, { fields: [chunk.userId], references: [user.id] }),
    outgoingConnections: many(chunkConnection, { relationName: "source" }),
    incomingConnections: many(chunkConnection, { relationName: "target" })
}));

export const chunkConnectionRelations = relations(chunkConnection, ({ one }) => ({
    source: one(chunk, {
        fields: [chunkConnection.sourceId],
        references: [chunk.id],
        relationName: "source"
    }),
    target: one(chunk, {
        fields: [chunkConnection.targetId],
        references: [chunk.id],
        relationName: "target"
    })
}));
