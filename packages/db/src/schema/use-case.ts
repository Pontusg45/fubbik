import { relations } from "drizzle-orm";
import { index, integer, pgTable, text, timestamp, uniqueIndex, type AnyPgColumn } from "drizzle-orm/pg-core";
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
        parentId: text("parent_id").references((): AnyPgColumn => useCase.id, { onDelete: "cascade" }),
        createdAt: timestamp("created_at").defaultNow().notNull(),
        updatedAt: timestamp("updated_at")
            .defaultNow()
            .$onUpdate(() => new Date())
            .notNull()
    },
    table => [
        index("use_case_userId_idx").on(table.userId),
        uniqueIndex("use_case_user_name_idx").on(table.userId, table.name),
        index("use_case_parentId_idx").on(table.parentId)
    ]
);

export const useCaseRelations = relations(useCase, ({ one, many }) => ({
    user: one(user, { fields: [useCase.userId], references: [user.id] }),
    codebase: one(codebase, { fields: [useCase.codebaseId], references: [codebase.id] }),
    parent: one(useCase, { fields: [useCase.parentId], references: [useCase.id], relationName: "children" }),
    children: many(useCase, { relationName: "children" })
}));
