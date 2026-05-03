import { relations } from "drizzle-orm";
import { index, integer, jsonb, pgTable, primaryKey, text, timestamp, uniqueIndex } from "drizzle-orm/pg-core";

import { user } from "./auth";
import { chunk } from "./chunk";
import { codebase } from "./codebase";

export const feature = pgTable(
    "feature",
    {
        id: text("id").primaryKey(),
        name: text("name").notNull(),
        description: text("description"),
        priority: integer("priority").notNull(),
        status: text("status").notNull().default("inactive"),
        color: text("color"),
        userId: text("user_id")
            .notNull()
            .references(() => user.id, { onDelete: "cascade" }),
        createdAt: timestamp("created_at").defaultNow().notNull(),
        updatedAt: timestamp("updated_at").defaultNow().notNull().$onUpdate(() => new Date())
    },
    table => [
        uniqueIndex("feature_user_name_idx").on(table.userId, table.name),
        uniqueIndex("feature_user_priority_idx").on(table.userId, table.priority)
    ]
);

export const featureCodebase = pgTable(
    "feature_codebase",
    {
        featureId: text("feature_id")
            .notNull()
            .references(() => feature.id, { onDelete: "cascade" }),
        codebaseId: text("codebase_id")
            .notNull()
            .references(() => codebase.id, { onDelete: "cascade" })
    },
    table => [primaryKey({ columns: [table.featureId, table.codebaseId] })]
);

export const chunkFeatureDelta = pgTable(
    "chunk_feature_delta",
    {
        id: text("id").primaryKey(),
        chunkId: text("chunk_id")
            .notNull()
            .references(() => chunk.id, { onDelete: "cascade" }),
        featureId: text("feature_id")
            .notNull()
            .references(() => feature.id, { onDelete: "cascade" }),
        delta: jsonb("delta").notNull().$type<Record<string, unknown>>(),
        createdAt: timestamp("created_at").defaultNow().notNull(),
        updatedAt: timestamp("updated_at").defaultNow().notNull().$onUpdate(() => new Date())
    },
    table => [
        uniqueIndex("chunk_feature_delta_chunk_feature_idx").on(table.chunkId, table.featureId),
        index("chunk_feature_delta_feature_idx").on(table.featureId)
    ]
);

export const userActiveFeature = pgTable(
    "user_active_feature",
    {
        userId: text("user_id")
            .notNull()
            .references(() => user.id, { onDelete: "cascade" }),
        featureId: text("feature_id")
            .notNull()
            .references(() => feature.id, { onDelete: "cascade" })
    },
    table => [primaryKey({ columns: [table.userId, table.featureId] })]
);

export const featureRelations = relations(feature, ({ one, many }) => ({
    user: one(user, { fields: [feature.userId], references: [user.id] }),
    codebases: many(featureCodebase),
    deltas: many(chunkFeatureDelta),
    activeUsers: many(userActiveFeature)
}));

export const featureCodebaseRelations = relations(featureCodebase, ({ one }) => ({
    feature: one(feature, { fields: [featureCodebase.featureId], references: [feature.id] }),
    codebase: one(codebase, { fields: [featureCodebase.codebaseId], references: [codebase.id] })
}));

export const chunkFeatureDeltaRelations = relations(chunkFeatureDelta, ({ one }) => ({
    chunk: one(chunk, { fields: [chunkFeatureDelta.chunkId], references: [chunk.id] }),
    feature: one(feature, { fields: [chunkFeatureDelta.featureId], references: [feature.id] })
}));

export const userActiveFeatureRelations = relations(userActiveFeature, ({ one }) => ({
    user: one(user, { fields: [userActiveFeature.userId], references: [user.id] }),
    feature: one(feature, { fields: [userActiveFeature.featureId], references: [feature.id] })
}));
