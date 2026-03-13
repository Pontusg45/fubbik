import { relations } from "drizzle-orm";
import { index, pgTable, text } from "drizzle-orm/pg-core";
import { chunk } from "./chunk";

export const chunkFileRef = pgTable(
    "chunk_file_ref",
    {
        id: text("id").primaryKey(),
        chunkId: text("chunk_id")
            .notNull()
            .references(() => chunk.id, { onDelete: "cascade" }),
        path: text("path").notNull(),
        anchor: text("anchor"),
        relation: text("relation").notNull().default("documents")
    },
    table => [
        index("chunk_file_ref_chunkId_idx").on(table.chunkId),
        index("chunk_file_ref_path_idx").on(table.path)
    ]
);

export const chunkFileRefRelations = relations(chunkFileRef, ({ one }) => ({
    chunk: one(chunk, { fields: [chunkFileRef.chunkId], references: [chunk.id] })
}));
