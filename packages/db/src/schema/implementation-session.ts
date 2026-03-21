import { boolean, index, jsonb, pgTable, primaryKey, text, timestamp } from "drizzle-orm/pg-core";
import { relations } from "drizzle-orm";
import { user } from "./auth";
import { chunk } from "./chunk";
import { codebase } from "./codebase";
import { plan } from "./plan";
import { requirement } from "./requirement";

export const implementationSession = pgTable(
    "implementation_session",
    {
        id: text("id").primaryKey(),
        title: text("title").notNull(),
        status: text("status").notNull().default("in_progress"),
        userId: text("user_id").notNull().references(() => user.id, { onDelete: "cascade" }),
        codebaseId: text("codebase_id").references(() => codebase.id, { onDelete: "set null" }),
        planId: text("plan_id").references(() => plan.id, { onDelete: "set null" }),
        prUrl: text("pr_url"),
        reviewBrief: text("review_brief"),
        createdAt: timestamp("created_at").defaultNow().notNull(),
        updatedAt: timestamp("updated_at").defaultNow().$onUpdate(() => new Date()).notNull(),
        completedAt: timestamp("completed_at"),
        reviewedAt: timestamp("reviewed_at")
    },
    table => [
        index("impl_session_userId_idx").on(table.userId),
        index("impl_session_codebaseId_idx").on(table.codebaseId),
        index("impl_session_status_idx").on(table.status),
        index("impl_session_planId_idx").on(table.planId)
    ]
);

export const sessionChunkRef = pgTable(
    "session_chunk_ref",
    {
        sessionId: text("session_id").notNull().references(() => implementationSession.id, { onDelete: "cascade" }),
        chunkId: text("chunk_id").notNull().references(() => chunk.id, { onDelete: "cascade" }),
        reason: text("reason").notNull()
    },
    table => [primaryKey({ columns: [table.sessionId, table.chunkId] })]
);

export const sessionAssumption = pgTable(
    "session_assumption",
    {
        id: text("id").primaryKey(),
        sessionId: text("session_id").notNull().references(() => implementationSession.id, { onDelete: "cascade" }),
        description: text("description").notNull(),
        resolved: boolean("resolved").notNull().default(false),
        resolution: text("resolution")
    },
    table => [index("assumption_sessionId_idx").on(table.sessionId)]
);

export const sessionRequirementRef = pgTable(
    "session_requirement_ref",
    {
        sessionId: text("session_id").notNull().references(() => implementationSession.id, { onDelete: "cascade" }),
        requirementId: text("requirement_id").notNull().references(() => requirement.id, { onDelete: "cascade" }),
        stepsAddressed: jsonb("steps_addressed").notNull().default([])
    },
    table => [primaryKey({ columns: [table.sessionId, table.requirementId] })]
);

// Relations
export const implementationSessionRelations = relations(implementationSession, ({ one, many }) => ({
    user: one(user, { fields: [implementationSession.userId], references: [user.id] }),
    codebase: one(codebase, { fields: [implementationSession.codebaseId], references: [codebase.id] }),
    plan: one(plan, { fields: [implementationSession.planId], references: [plan.id] }),
    chunkRefs: many(sessionChunkRef),
    assumptions: many(sessionAssumption),
    requirementRefs: many(sessionRequirementRef)
}));

export const sessionChunkRefRelations = relations(sessionChunkRef, ({ one }) => ({
    session: one(implementationSession, { fields: [sessionChunkRef.sessionId], references: [implementationSession.id] }),
    chunk: one(chunk, { fields: [sessionChunkRef.chunkId], references: [chunk.id] })
}));

export const sessionAssumptionRelations = relations(sessionAssumption, ({ one }) => ({
    session: one(implementationSession, { fields: [sessionAssumption.sessionId], references: [implementationSession.id] })
}));

export const sessionRequirementRefRelations = relations(sessionRequirementRef, ({ one }) => ({
    session: one(implementationSession, { fields: [sessionRequirementRef.sessionId], references: [implementationSession.id] }),
    requirement: one(requirement, { fields: [sessionRequirementRef.requirementId], references: [requirement.id] })
}));
