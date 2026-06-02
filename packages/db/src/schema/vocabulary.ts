import { relations, sql } from "drizzle-orm";
import { index, jsonb, pgTable, text, timestamp, uniqueIndex } from "drizzle-orm/pg-core";

import { user } from "./auth";
import { space } from "./space";

export const vocabularyEntry = pgTable(
    "vocabulary_entry",
    {
        id: text("id").primaryKey(),
        word: text("word").notNull(),
        definition: text("definition"),
        category: text("category").notNull(),
        expects: jsonb("expects").$type<string[]>(),
        spaceId: text("space_id")
            .notNull()
            .references(() => space.id, { onDelete: "cascade" }),
        userId: text("user_id").references(() => user.id, { onDelete: "set null" }),
        createdAt: timestamp("created_at").defaultNow().notNull(),
        updatedAt: timestamp("updated_at")
            .defaultNow()
            .$onUpdate(() => new Date())
            .notNull()
    },
    table => [
        uniqueIndex("vocabulary_space_word_cat_idx").on(table.spaceId, table.category, sql`lower(${table.word})`),
        index("vocabulary_spaceId_idx").on(table.spaceId)
    ]
);

export const vocabularyEntryRelations = relations(vocabularyEntry, ({ one }) => ({
    space: one(space, { fields: [vocabularyEntry.spaceId], references: [space.id] }),
    user: one(user, { fields: [vocabularyEntry.userId], references: [user.id] })
}));
