import { relations, sql } from "drizzle-orm";
import { index, jsonb, pgTable, text, timestamp, uniqueIndex } from "drizzle-orm/pg-core";
import { user } from "./auth";
import { codebase } from "./codebase";

export const vocabularyEntry = pgTable(
    "vocabulary_entry",
    {
        id: text("id").primaryKey(),
        word: text("word").notNull(),
        category: text("category").notNull(),
        expects: jsonb("expects").$type<string[]>(),
        codebaseId: text("codebase_id")
            .notNull()
            .references(() => codebase.id, { onDelete: "cascade" }),
        userId: text("user_id").references(() => user.id, { onDelete: "set null" }),
        createdAt: timestamp("created_at").defaultNow().notNull(),
        updatedAt: timestamp("updated_at")
            .defaultNow()
            .$onUpdate(() => new Date())
            .notNull()
    },
    table => [
        uniqueIndex("vocabulary_codebase_word_cat_idx").on(
            table.codebaseId,
            table.category,
            sql`lower(${table.word})`
        ),
        index("vocabulary_codebaseId_idx").on(table.codebaseId)
    ]
);

export const vocabularyEntryRelations = relations(vocabularyEntry, ({ one }) => ({
    codebase: one(codebase, { fields: [vocabularyEntry.codebaseId], references: [codebase.id] }),
    user: one(user, { fields: [vocabularyEntry.userId], references: [user.id] })
}));
