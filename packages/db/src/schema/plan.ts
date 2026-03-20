import { relations } from "drizzle-orm";
import { index, integer, pgTable, text, timestamp, type AnyPgColumn } from "drizzle-orm/pg-core";
import { user } from "./auth";
import { chunk } from "./chunk";
import { codebase } from "./codebase";

export const plan = pgTable(
    "plan",
    {
        id: text("id").primaryKey(),
        title: text("title").notNull(),
        description: text("description"),
        status: text("status").notNull().default("draft"),
        userId: text("user_id")
            .notNull()
            .references(() => user.id, { onDelete: "cascade" }),
        codebaseId: text("codebase_id").references(() => codebase.id, { onDelete: "set null" }),
        createdAt: timestamp("created_at").defaultNow().notNull(),
        updatedAt: timestamp("updated_at")
            .defaultNow()
            .$onUpdate(() => new Date())
            .notNull()
    },
    table => [
        index("plan_userId_idx").on(table.userId),
        index("plan_codebaseId_idx").on(table.codebaseId)
    ]
);

export const planStep = pgTable(
    "plan_step",
    {
        id: text("id").primaryKey(),
        planId: text("plan_id")
            .notNull()
            .references(() => plan.id, { onDelete: "cascade" }),
        description: text("description").notNull(),
        status: text("status").notNull().default("pending"),
        order: integer("order").notNull().default(0),
        parentStepId: text("parent_step_id").references((): AnyPgColumn => planStep.id, {
            onDelete: "cascade"
        }),
        note: text("note"),
        chunkId: text("chunk_id").references(() => chunk.id, { onDelete: "set null" }),
        createdAt: timestamp("created_at").defaultNow().notNull(),
        updatedAt: timestamp("updated_at")
            .defaultNow()
            .$onUpdate(() => new Date())
            .notNull()
    },
    table => [
        index("planStep_planId_idx").on(table.planId),
        index("planStep_parentStepId_idx").on(table.parentStepId)
    ]
);

export const planChunkRef = pgTable(
    "plan_chunk_ref",
    {
        id: text("id").primaryKey(),
        planId: text("plan_id")
            .notNull()
            .references(() => plan.id, { onDelete: "cascade" }),
        chunkId: text("chunk_id")
            .notNull()
            .references(() => chunk.id, { onDelete: "cascade" }),
        relation: text("relation").notNull().default("context"),
        createdAt: timestamp("created_at").defaultNow().notNull()
    },
    table => [
        index("planChunkRef_planId_idx").on(table.planId),
        index("planChunkRef_chunkId_idx").on(table.chunkId)
    ]
);

export const planRelations = relations(plan, ({ one, many }) => ({
    user: one(user, { fields: [plan.userId], references: [user.id] }),
    codebase: one(codebase, { fields: [plan.codebaseId], references: [codebase.id] }),
    steps: many(planStep),
    chunkRefs: many(planChunkRef)
}));

export const planStepRelations = relations(planStep, ({ one, many }) => ({
    plan: one(plan, { fields: [planStep.planId], references: [plan.id] }),
    parentStep: one(planStep, {
        fields: [planStep.parentStepId],
        references: [planStep.id],
        relationName: "children"
    }),
    children: many(planStep, { relationName: "children" }),
    chunk: one(chunk, { fields: [planStep.chunkId], references: [chunk.id] })
}));

export const planChunkRefRelations = relations(planChunkRef, ({ one }) => ({
    plan: one(plan, { fields: [planChunkRef.planId], references: [plan.id] }),
    chunk: one(chunk, { fields: [planChunkRef.chunkId], references: [chunk.id] })
}));
