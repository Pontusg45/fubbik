import { relations } from "drizzle-orm";
import { index, pgTable, text } from "drizzle-orm/pg-core";
import { chunk } from "./chunk";

export const chunkAppliesTo = pgTable(
    "chunk_applies_to",
    {
        id: text("id").primaryKey(),
        chunkId: text("chunk_id")
            .notNull()
            .references(() => chunk.id, { onDelete: "cascade" }),
        pattern: text("pattern").notNull(),
        note: text("note")
    },
    table => [index("chunk_applies_to_chunkId_idx").on(table.chunkId)]
);

export const chunkAppliesToRelations = relations(chunkAppliesTo, ({ one }) => ({
    chunk: one(chunk, { fields: [chunkAppliesTo.chunkId], references: [chunk.id] })
}));
