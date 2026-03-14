import { relations } from "drizzle-orm";
import { index, jsonb, pgTable, primaryKey, text, timestamp } from "drizzle-orm/pg-core";
import { user } from "./auth";
import { chunk } from "./chunk";
import { codebase } from "./codebase";
import { useCase } from "./use-case";

export interface RequirementStep {
    keyword: "given" | "when" | "then" | "and" | "but";
    text: string;
    params?: Record<string, string>;
}

export const requirement = pgTable(
    "requirement",
    {
        id: text("id").primaryKey(),
        title: text("title").notNull(),
        description: text("description"),
        steps: jsonb("steps").$type<RequirementStep[]>().notNull(),
        status: text("status").notNull().default("untested"),
        priority: text("priority"),
        codebaseId: text("codebase_id").references(() => codebase.id, { onDelete: "set null" }),
        userId: text("user_id")
            .notNull()
            .references(() => user.id, { onDelete: "cascade" }),
        createdAt: timestamp("created_at").defaultNow().notNull(),
        updatedAt: timestamp("updated_at")
            .defaultNow()
            .$onUpdate(() => new Date())
            .notNull(),
        origin: text("origin").notNull().default("human"),
        reviewStatus: text("review_status").notNull().default("approved"),
        useCaseId: text("use_case_id").references(() => useCase.id, { onDelete: "set null" }),
        reviewedBy: text("reviewed_by").references(() => user.id, { onDelete: "set null" }),
        reviewedAt: timestamp("reviewed_at")
    },
    table => [
        index("requirement_userId_idx").on(table.userId),
        index("requirement_codebaseId_idx").on(table.codebaseId),
        index("requirement_status_idx").on(table.status)
    ]
);

export const requirementChunk = pgTable(
    "requirement_chunk",
    {
        requirementId: text("requirement_id")
            .notNull()
            .references(() => requirement.id, { onDelete: "cascade" }),
        chunkId: text("chunk_id")
            .notNull()
            .references(() => chunk.id, { onDelete: "cascade" })
    },
    table => [primaryKey({ columns: [table.requirementId, table.chunkId] })]
);

export const requirementRelations = relations(requirement, ({ one, many }) => ({
    user: one(user, { fields: [requirement.userId], references: [user.id] }),
    codebase: one(codebase, { fields: [requirement.codebaseId], references: [codebase.id] }),
    useCase: one(useCase, { fields: [requirement.useCaseId], references: [useCase.id] }),
    requirementChunks: many(requirementChunk)
}));

export const requirementChunkRelations = relations(requirementChunk, ({ one }) => ({
    requirement: one(requirement, {
        fields: [requirementChunk.requirementId],
        references: [requirement.id]
    }),
    chunk: one(chunk, {
        fields: [requirementChunk.chunkId],
        references: [chunk.id]
    })
}));
