import { relations } from "drizzle-orm";
import { index, integer, jsonb, pgTable, text, timestamp, uniqueIndex } from "drizzle-orm/pg-core";

import { chunk } from "./chunk";
import { codebase } from "./codebase";
import { requirement } from "./requirement";
import { user } from "./auth";

/**
 * Plan: the central unit of work. Holds description, linked requirements,
 * structured analyze fields, and enriched tasks.
 */
export const plan = pgTable(
    "plan",
    {
        id: text("id").primaryKey().$defaultFn(() => crypto.randomUUID()),
        title: text("title").notNull(),
        description: text("description"),
        status: text("status").notNull().default("draft"),
        // draft | analyzing | ready | in_progress | completed | archived — labels, ungated
        userId: text("user_id")
            .notNull()
            .references(() => user.id, { onDelete: "cascade" }),
        codebaseId: text("codebase_id").references(() => codebase.id, { onDelete: "set null" }),
        createdAt: timestamp("created_at").notNull().defaultNow(),
        updatedAt: timestamp("updated_at")
            .notNull()
            .defaultNow()
            .$onUpdate(() => new Date()),
        completedAt: timestamp("completed_at"),
    },
    table => [
        index("plan_userId_idx").on(table.userId),
        index("plan_codebaseId_idx").on(table.codebaseId),
    ],
);

export const planRequirement = pgTable(
    "plan_requirement",
    {
        id: text("id").primaryKey().$defaultFn(() => crypto.randomUUID()),
        planId: text("plan_id")
            .notNull()
            .references(() => plan.id, { onDelete: "cascade" }),
        requirementId: text("requirement_id")
            .notNull()
            .references(() => requirement.id, { onDelete: "cascade" }),
        order: integer("order").notNull().default(0),
        createdAt: timestamp("created_at").notNull().defaultNow(),
    },
    t => [
        uniqueIndex("plan_requirement_unique_idx").on(t.planId, t.requirementId),
        index("plan_requirement_planId_idx").on(t.planId),
    ],
);

/**
 * plan_analyze_item: one discriminated table holding all five analyze kinds
 * (chunk, file, risk, assumption, question).
 */
export const planAnalyzeItem = pgTable(
    "plan_analyze_item",
    {
        id: text("id").primaryKey().$defaultFn(() => crypto.randomUUID()),
        planId: text("plan_id")
            .notNull()
            .references(() => plan.id, { onDelete: "cascade" }),
        kind: text("kind").notNull(), // chunk | file | risk | assumption | question
        order: integer("order").notNull().default(0),
        chunkId: text("chunk_id").references(() => chunk.id, { onDelete: "cascade" }),
        filePath: text("file_path"),
        text: text("text"),
        metadata: jsonb("metadata").notNull().default({}),
        createdAt: timestamp("created_at").notNull().defaultNow(),
        updatedAt: timestamp("updated_at")
            .notNull()
            .defaultNow()
            .$onUpdate(() => new Date()),
    },
    table => [
        index("plan_analyze_item_planId_idx").on(table.planId),
        index("plan_analyze_item_chunkId_idx").on(table.chunkId),
    ],
);

export const planTask = pgTable(
    "plan_task",
    {
        id: text("id").primaryKey().$defaultFn(() => crypto.randomUUID()),
        planId: text("plan_id")
            .notNull()
            .references(() => plan.id, { onDelete: "cascade" }),
        title: text("title").notNull(),
        description: text("description"),
        acceptanceCriteria: jsonb("acceptance_criteria").notNull().default([]),
        status: text("status").notNull().default("pending"),
        // pending | in_progress | done | skipped | blocked
        order: integer("order").notNull().default(0),
        createdAt: timestamp("created_at").notNull().defaultNow(),
        updatedAt: timestamp("updated_at")
            .notNull()
            .defaultNow()
            .$onUpdate(() => new Date()),
    },
    table => [index("plan_task_planId_idx").on(table.planId)],
);

export const planTaskChunk = pgTable(
    "plan_task_chunk",
    {
        id: text("id").primaryKey().$defaultFn(() => crypto.randomUUID()),
        taskId: text("task_id")
            .notNull()
            .references(() => planTask.id, { onDelete: "cascade" }),
        chunkId: text("chunk_id")
            .notNull()
            .references(() => chunk.id, { onDelete: "cascade" }),
        relation: text("relation").notNull(), // context | created | modified
        createdAt: timestamp("created_at").notNull().defaultNow(),
    },
    t => [
        uniqueIndex("plan_task_chunk_unique_idx").on(t.taskId, t.chunkId, t.relation),
        index("plan_task_chunk_taskId_idx").on(t.taskId),
    ],
);

export const planTaskDependency = pgTable(
    "plan_task_dependency",
    {
        id: text("id").primaryKey().$defaultFn(() => crypto.randomUUID()),
        taskId: text("task_id")
            .notNull()
            .references(() => planTask.id, { onDelete: "cascade" }),
        dependsOnTaskId: text("depends_on_task_id")
            .notNull()
            .references(() => planTask.id, { onDelete: "cascade" }),
        createdAt: timestamp("created_at").notNull().defaultNow(),
    },
    t => [
        uniqueIndex("plan_task_dependency_unique_idx").on(t.taskId, t.dependsOnTaskId),
        index("plan_task_dependency_taskId_idx").on(t.taskId),
    ],
);

// Relations
export const planRelations = relations(plan, ({ many, one }) => ({
    requirements: many(planRequirement),
    analyzeItems: many(planAnalyzeItem),
    tasks: many(planTask),
    codebase: one(codebase, { fields: [plan.codebaseId], references: [codebase.id] }),
    user: one(user, { fields: [plan.userId], references: [user.id] }),
}));

export const planRequirementRelations = relations(planRequirement, ({ one }) => ({
    plan: one(plan, { fields: [planRequirement.planId], references: [plan.id] }),
    requirement: one(requirement, { fields: [planRequirement.requirementId], references: [requirement.id] }),
}));

export const planAnalyzeItemRelations = relations(planAnalyzeItem, ({ one }) => ({
    plan: one(plan, { fields: [planAnalyzeItem.planId], references: [plan.id] }),
    chunk: one(chunk, { fields: [planAnalyzeItem.chunkId], references: [chunk.id] }),
}));

export const planTaskRelations = relations(planTask, ({ many, one }) => ({
    plan: one(plan, { fields: [planTask.planId], references: [plan.id] }),
    chunks: many(planTaskChunk),
    dependsOn: many(planTaskDependency, { relationName: "blockedBy" }),
    dependents: many(planTaskDependency, { relationName: "blocks" }),
}));

export const planTaskChunkRelations = relations(planTaskChunk, ({ one }) => ({
    task: one(planTask, { fields: [planTaskChunk.taskId], references: [planTask.id] }),
    chunk: one(chunk, { fields: [planTaskChunk.chunkId], references: [chunk.id] }),
}));

export const planTaskDependencyRelations = relations(planTaskDependency, ({ one }) => ({
    task: one(planTask, {
        fields: [planTaskDependency.taskId],
        references: [planTask.id],
        relationName: "blockedBy",
    }),
    dependsOnTask: one(planTask, {
        fields: [planTaskDependency.dependsOnTaskId],
        references: [planTask.id],
        relationName: "blocks",
    }),
}));

// Inferred types
export type Plan = typeof plan.$inferSelect;
export type NewPlan = typeof plan.$inferInsert;
export type PlanRequirement = typeof planRequirement.$inferSelect;
export type NewPlanRequirement = typeof planRequirement.$inferInsert;
export type PlanAnalyzeItem = typeof planAnalyzeItem.$inferSelect;
export type NewPlanAnalyzeItem = typeof planAnalyzeItem.$inferInsert;
export type PlanTask = typeof planTask.$inferSelect;
export type NewPlanTask = typeof planTask.$inferInsert;
export type PlanTaskChunk = typeof planTaskChunk.$inferSelect;
export type NewPlanTaskChunk = typeof planTaskChunk.$inferInsert;
export type PlanTaskDependency = typeof planTaskDependency.$inferSelect;
export type NewPlanTaskDependency = typeof planTaskDependency.$inferInsert;

export type PlanStatus = "draft" | "analyzing" | "ready" | "in_progress" | "completed" | "archived";
export type PlanTaskStatus = "pending" | "in_progress" | "done" | "skipped" | "blocked";
export type PlanAnalyzeKind = "chunk" | "file" | "risk" | "assumption" | "question";
export type PlanTaskChunkRelation = "context" | "created" | "modified";
