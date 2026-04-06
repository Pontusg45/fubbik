import { relations } from "drizzle-orm";
import { index, pgTable, text, timestamp } from "drizzle-orm/pg-core";

import { user } from "./auth";
import { chunk } from "./chunk";
import { codebase } from "./codebase";

export const chunkStaleness = pgTable(
    "chunk_staleness",
    {
        id: text("id").primaryKey(),
        chunkId: text("chunk_id")
            .notNull()
            .references(() => chunk.id, { onDelete: "cascade" }),
        reason: text("reason").notNull(), // "file_changed" | "age" | "diverged_duplicate"
        detail: text("detail"),
        relatedChunkId: text("related_chunk_id").references(() => chunk.id, { onDelete: "cascade" }),
        detectedAt: timestamp("detected_at").defaultNow().notNull(),
        dismissedAt: timestamp("dismissed_at"),
        dismissedBy: text("dismissed_by").references(() => user.id, { onDelete: "set null" }),
        suppressPair: text("suppress_pair")
    },
    table => [
        index("chunk_staleness_chunkId_idx").on(table.chunkId),
        index("chunk_staleness_reason_idx").on(table.reason),
        index("chunk_staleness_dismissedAt_idx").on(table.dismissedAt)
    ]
);

export const chunkStalenessRelations = relations(chunkStaleness, ({ one }) => ({
    chunk: one(chunk, {
        fields: [chunkStaleness.chunkId],
        references: [chunk.id]
    }),
    relatedChunk: one(chunk, {
        fields: [chunkStaleness.relatedChunkId],
        references: [chunk.id]
    })
}));

export const stalenessScan = pgTable(
    "staleness_scan",
    {
        id: text("id").primaryKey(),
        codebaseId: text("codebase_id")
            .notNull()
            .references(() => codebase.id, { onDelete: "cascade" }),
        lastCommitSha: text("last_commit_sha").notNull(),
        scannedAt: timestamp("scanned_at").defaultNow().notNull()
    },
    table => [index("staleness_scan_codebaseId_idx").on(table.codebaseId)]
);
