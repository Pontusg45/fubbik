import { relations } from "drizzle-orm";
import { index, pgTable, primaryKey, text, timestamp, uniqueIndex } from "drizzle-orm/pg-core";

import { user } from "./auth";
import { codebase } from "./codebase";

export const workspace = pgTable(
    "workspace",
    {
        id: text("id").primaryKey(),
        name: text("name").notNull(),
        description: text("description"),
        userId: text("user_id")
            .notNull()
            .references(() => user.id, { onDelete: "cascade" }),
        createdAt: timestamp("created_at").defaultNow().notNull(),
        updatedAt: timestamp("updated_at")
            .defaultNow()
            .$onUpdate(() => new Date())
            .notNull()
    },
    table => [
        uniqueIndex("workspace_user_name_idx").on(table.userId, table.name),
        index("workspace_userId_idx").on(table.userId)
    ]
);

export const workspaceCodebase = pgTable(
    "workspace_codebase",
    {
        workspaceId: text("workspace_id")
            .notNull()
            .references(() => workspace.id, { onDelete: "cascade" }),
        codebaseId: text("codebase_id")
            .notNull()
            .references(() => codebase.id, { onDelete: "cascade" })
    },
    table => [
        primaryKey({ columns: [table.workspaceId, table.codebaseId] }),
        index("workspace_codebase_workspaceId_idx").on(table.workspaceId),
        index("workspace_codebase_codebaseId_idx").on(table.codebaseId)
    ]
);

export const workspaceRelations = relations(workspace, ({ one, many }) => ({
    user: one(user, { fields: [workspace.userId], references: [user.id] }),
    workspaceCodebases: many(workspaceCodebase)
}));

export const workspaceCodebaseRelations = relations(workspaceCodebase, ({ one }) => ({
    workspace: one(workspace, { fields: [workspaceCodebase.workspaceId], references: [workspace.id] }),
    codebase: one(codebase, { fields: [workspaceCodebase.codebaseId], references: [codebase.id] })
}));
