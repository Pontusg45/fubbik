import { relations } from "drizzle-orm";
import { index, integer, pgTable, text, timestamp, uniqueIndex } from "drizzle-orm/pg-core";
import { user } from "./auth";
import { codebase } from "./codebase";

export const useCase = pgTable(
    "use_case",
    {
        id: text("id").primaryKey(),
        name: text("name").notNull(),
        description: text("description"),
        codebaseId: text("codebase_id").references(() => codebase.id, { onDelete: "set null" }),
        userId: text("user_id")
            .notNull()
            .references(() => user.id, { onDelete: "cascade" }),
        order: integer("order").notNull().default(0),
        createdAt: timestamp("created_at").defaultNow().notNull(),
        updatedAt: timestamp("updated_at")
            .defaultNow()
            .$onUpdate(() => new Date())
            .notNull()
    },
    table => [
        index("use_case_userId_idx").on(table.userId),
        uniqueIndex("use_case_user_name_idx").on(table.userId, table.name)
    ]
);

export const useCaseRelations = relations(useCase, ({ one }) => ({
    user: one(user, { fields: [useCase.userId], references: [user.id] }),
    codebase: one(codebase, { fields: [useCase.codebaseId], references: [codebase.id] })
}));
