import { relations } from "drizzle-orm";
import { index, jsonb, pgTable, text, timestamp } from "drizzle-orm/pg-core";

import { chunk } from "./chunk";
import { user } from "./auth";

export const chunkProposal = pgTable(
    "chunk_proposal",
    {
        id: text("id").primaryKey().$defaultFn(() => crypto.randomUUID()),
        chunkId: text("chunk_id")
            .notNull()
            .references(() => chunk.id, { onDelete: "cascade" }),
        changes: jsonb("changes").notNull(),
        reason: text("reason"),
        status: text("status").notNull().default("pending"),
        proposedBy: text("proposed_by").notNull(),
        reviewedBy: text("reviewed_by").references(() => user.id, { onDelete: "set null" }),
        reviewedAt: timestamp("reviewed_at"),
        reviewNote: text("review_note"),
        createdAt: timestamp("created_at").notNull().defaultNow(),
    },
    table => [
        index("chunk_proposal_chunkId_idx").on(table.chunkId),
        index("chunk_proposal_status_idx").on(table.status),
        index("chunk_proposal_chunkId_status_idx").on(table.chunkId, table.status),
    ],
);

export const chunkProposalRelations = relations(chunkProposal, ({ one }) => ({
    chunk: one(chunk, { fields: [chunkProposal.chunkId], references: [chunk.id] }),
    reviewer: one(user, { fields: [chunkProposal.reviewedBy], references: [user.id] }),
}));

export type ChunkProposal = typeof chunkProposal.$inferSelect;
export type NewChunkProposal = typeof chunkProposal.$inferInsert;

export interface ProposedChanges {
    title?: string;
    content?: string;
    type?: string;
    tags?: string[];
    rationale?: string;
    alternatives?: string[];
    consequences?: string;
    scope?: Record<string, string>;
}
