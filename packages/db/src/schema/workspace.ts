import { relations } from "drizzle-orm";
import { index, pgTable, primaryKey, text, timestamp, uniqueIndex } from "drizzle-orm/pg-core";

import { user } from "./auth";
import { space } from "./space";

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
    table => [uniqueIndex("workspace_user_name_idx").on(table.userId, table.name), index("workspace_userId_idx").on(table.userId)]
);

export const workspaceSpace = pgTable(
    "workspace_space",
    {
        workspaceId: text("workspace_id")
            .notNull()
            .references(() => workspace.id, { onDelete: "cascade" }),
        spaceId: text("space_id")
            .notNull()
            .references(() => space.id, { onDelete: "cascade" })
    },
    table => [
        primaryKey({ columns: [table.workspaceId, table.spaceId] }),
        index("workspace_space_workspaceId_idx").on(table.workspaceId),
        index("workspace_space_spaceId_idx").on(table.spaceId)
    ]
);

export const workspaceRelations = relations(workspace, ({ one, many }) => ({
    user: one(user, { fields: [workspace.userId], references: [user.id] }),
    workspaceSpaces: many(workspaceSpace)
}));

export const workspaceSpaceRelations = relations(workspaceSpace, ({ one }) => ({
    workspace: one(workspace, { fields: [workspaceSpace.workspaceId], references: [workspace.id] }),
    space: one(space, { fields: [workspaceSpace.spaceId], references: [space.id] })
}));
