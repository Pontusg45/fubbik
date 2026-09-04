import { relations, sql } from "drizzle-orm";
import { bigint, check, foreignKey, index, jsonb, pgTable, text, timestamp, unique, uniqueIndex } from "drizzle-orm/pg-core";

import { plan, planTask } from "./plan";

export const agentRun = pgTable(
    "agent_run",
    {
        id: text("id").primaryKey(),
        planId: text("plan_id")
            .notNull()
            .references(() => plan.id, { onDelete: "cascade" }),
        parentRunId: text("parent_run_id"),
        handle: text("handle").notNull(),
        externalKey: text("external_key"),
        status: text("status").notNull().default("active"),
        capabilities: jsonb("capabilities").$type<string[]>().notNull().default([]),
        metadata: jsonb("metadata").$type<Record<string, unknown>>().notNull().default({}),
        lastAckSequence: bigint("last_ack_sequence", { mode: "number" }).notNull().default(0),
        lastHeartbeatAt: timestamp("last_heartbeat_at").notNull().defaultNow(),
        createdAt: timestamp("created_at").notNull().defaultNow(),
        updatedAt: timestamp("updated_at").notNull().defaultNow()
    },
    table => [
        unique("agent_run_id_plan_id_unique").on(table.id, table.planId),
        check("agent_run_handle_check", sql`btrim(${table.handle}) <> ''`),
        check("agent_run_status_check", sql`${table.status} in ('active', 'finished', 'abandoned')`),
        check("agent_run_last_ack_sequence_check", sql`${table.lastAckSequence} >= 0`),
        uniqueIndex("agent_run_plan_external_key_unique_idx")
            .on(table.planId, table.externalKey)
            .where(sql`${table.externalKey} is not null`),
        index("agent_run_plan_status_idx").on(table.planId, table.status),
        index("agent_run_parent_idx").on(table.parentRunId),
        foreignKey({
            name: "agent_run_parent_same_plan_fk",
            columns: [table.parentRunId, table.planId],
            foreignColumns: [table.id, table.planId]
        }).onDelete("cascade")
    ]
);

export const planTaskClaim = pgTable(
    "plan_task_claim",
    {
        taskId: text("task_id").primaryKey(),
        planId: text("plan_id").notNull(),
        agentRunId: text("agent_run_id").notNull(),
        claimedAt: timestamp("claimed_at").notNull().defaultNow(),
        leaseExpiresAt: timestamp("lease_expires_at").notNull(),
        updatedAt: timestamp("updated_at").notNull().defaultNow()
    },
    table => [
        foreignKey({
            name: "plan_task_claim_task_same_plan_fk",
            columns: [table.taskId, table.planId],
            foreignColumns: [planTask.id, planTask.planId]
        }).onDelete("cascade"),
        foreignKey({
            name: "plan_task_claim_run_same_plan_fk",
            columns: [table.agentRunId, table.planId],
            foreignColumns: [agentRun.id, agentRun.planId]
        }).onDelete("cascade"),
        index("plan_task_claim_plan_lease_idx").on(table.planId, table.leaseExpiresAt)
    ]
);

export const coordinationEntry = pgTable(
    "coordination_entry",
    {
        id: text("id").primaryKey(),
        sequence: bigint("sequence", { mode: "number" }).generatedAlwaysAsIdentity().notNull().unique(),
        planId: text("plan_id")
            .notNull()
            .references(() => plan.id, { onDelete: "cascade" }),
        taskId: text("task_id"),
        authorRunId: text("author_run_id").notNull(),
        recipientRunId: text("recipient_run_id"),
        replyToId: text("reply_to_id"),
        kind: text("kind").notNull(),
        body: text("body").notNull(),
        metadata: jsonb("metadata").$type<Record<string, unknown>>().notNull().default({}),
        clientMutationId: text("client_mutation_id").notNull(),
        createdAt: timestamp("created_at").notNull().defaultNow()
    },
    table => [
        unique("coordination_entry_id_plan_id_unique").on(table.id, table.planId),
        unique("coordination_entry_author_mutation_unique").on(table.authorRunId, table.clientMutationId),
        check(
            "coordination_entry_kind_check",
            sql`${table.kind} in ('note', 'question', 'answer', 'progress', 'decision', 'handoff', 'artifact', 'system')`
        ),
        check("coordination_entry_body_check", sql`btrim(${table.body}) <> ''`),
        check("coordination_entry_client_mutation_id_check", sql`btrim(${table.clientMutationId}) <> ''`),
        foreignKey({
            name: "coordination_entry_task_same_plan_fk",
            columns: [table.taskId, table.planId],
            foreignColumns: [planTask.id, planTask.planId]
        }).onDelete("cascade"),
        foreignKey({
            name: "coordination_entry_author_same_plan_fk",
            columns: [table.authorRunId, table.planId],
            foreignColumns: [agentRun.id, agentRun.planId]
        }).onDelete("cascade"),
        foreignKey({
            name: "coordination_entry_recipient_same_plan_fk",
            columns: [table.recipientRunId, table.planId],
            foreignColumns: [agentRun.id, agentRun.planId]
        }).onDelete("cascade"),
        foreignKey({
            name: "coordination_entry_reply_same_plan_fk",
            columns: [table.replyToId, table.planId],
            foreignColumns: [table.id, table.planId]
        }).onDelete("cascade"),
        index("coordination_entry_plan_sequence_idx").on(table.planId, table.sequence),
        index("coordination_entry_recipient_sequence_idx").on(table.recipientRunId, table.sequence),
        index("coordination_entry_task_sequence_idx").on(table.taskId, table.sequence)
    ]
);

export const agentRunRelations = relations(agentRun, ({ one, many }) => ({
    plan: one(plan, { fields: [agentRun.planId], references: [plan.id] }),
    claims: many(planTaskClaim),
    entries: many(coordinationEntry, { relationName: "entryAuthor" })
}));

export const planTaskClaimRelations = relations(planTaskClaim, ({ one }) => ({
    task: one(planTask, { fields: [planTaskClaim.taskId], references: [planTask.id] }),
    run: one(agentRun, { fields: [planTaskClaim.agentRunId], references: [agentRun.id] })
}));

export const coordinationEntryRelations = relations(coordinationEntry, ({ one }) => ({
    author: one(agentRun, { fields: [coordinationEntry.authorRunId], references: [agentRun.id], relationName: "entryAuthor" }),
    recipient: one(agentRun, { fields: [coordinationEntry.recipientRunId], references: [agentRun.id], relationName: "entryRecipient" }),
    task: one(planTask, { fields: [coordinationEntry.taskId], references: [planTask.id] })
}));

export type AgentRun = typeof agentRun.$inferSelect;
export type PlanTaskClaim = typeof planTaskClaim.$inferSelect;
export type CoordinationEntry = typeof coordinationEntry.$inferSelect;
