import { relations } from "drizzle-orm";
import { index, integer, pgTable, primaryKey, text, timestamp, unique } from "drizzle-orm/pg-core";

import { user } from "./auth";
import { requirement } from "./requirement";
import { space } from "./space";

export const behaviorMatrix = pgTable(
    "behavior_matrix",
    {
        id: text("id").primaryKey(),
        name: text("name").notNull(),
        layer: text("layer").notNull(),
        description: text("description"),
        spaceId: text("space_id").references(() => space.id, { onDelete: "set null" }),
        userId: text("user_id")
            .notNull()
            .references(() => user.id, { onDelete: "cascade" }),
        createdAt: timestamp("created_at").defaultNow().notNull(),
        updatedAt: timestamp("updated_at")
            .defaultNow()
            .$onUpdate(() => new Date())
            .notNull()
    },
    table => [index("behavior_matrix_userId_idx").on(table.userId), index("behavior_matrix_layer_idx").on(table.layer)]
);

export const behaviorDimension = pgTable(
    "behavior_dimension",
    {
        id: text("id").primaryKey(),
        matrixId: text("matrix_id")
            .notNull()
            .references(() => behaviorMatrix.id, { onDelete: "cascade" }),
        name: text("name").notNull(),
        order: integer("order").notNull().default(0),
        createdAt: timestamp("created_at").defaultNow().notNull()
    },
    table => [
        unique("behavior_dimension_matrix_name").on(table.matrixId, table.name),
        index("behavior_dimension_matrixId_idx").on(table.matrixId)
    ]
);

export const behaviorRule = pgTable(
    "behavior_rule",
    {
        id: text("id").primaryKey(),
        matrixId: text("matrix_id")
            .notNull()
            .references(() => behaviorMatrix.id, { onDelete: "cascade" }),
        title: text("title").notNull(),
        description: text("description"),
        category: text("category"),
        order: integer("order").notNull().default(0),
        createdAt: timestamp("created_at").defaultNow().notNull(),
        updatedAt: timestamp("updated_at")
            .defaultNow()
            .$onUpdate(() => new Date())
            .notNull()
    },
    table => [index("behavior_rule_matrixId_idx").on(table.matrixId)]
);

export const behaviorCell = pgTable(
    "behavior_cell",
    {
        id: text("id").primaryKey(),
        ruleId: text("rule_id")
            .notNull()
            .references(() => behaviorRule.id, { onDelete: "cascade" }),
        dimensionId: text("dimension_id")
            .notNull()
            .references(() => behaviorDimension.id, { onDelete: "cascade" }),
        createdAt: timestamp("created_at").defaultNow().notNull()
    },
    table => [
        unique("behavior_cell_rule_dimension").on(table.ruleId, table.dimensionId),
        index("behavior_cell_ruleId_idx").on(table.ruleId),
        index("behavior_cell_dimensionId_idx").on(table.dimensionId)
    ]
);

export const behaviorCellRequirement = pgTable(
    "behavior_cell_requirement",
    {
        cellId: text("cell_id")
            .notNull()
            .references(() => behaviorCell.id, { onDelete: "cascade" }),
        requirementId: text("requirement_id")
            .notNull()
            .references(() => requirement.id, { onDelete: "cascade" })
    },
    table => [primaryKey({ columns: [table.cellId, table.requirementId] })]
);

export const behaviorMatrixRelations = relations(behaviorMatrix, ({ one, many }) => ({
    user: one(user, { fields: [behaviorMatrix.userId], references: [user.id] }),
    space: one(space, { fields: [behaviorMatrix.spaceId], references: [space.id] }),
    dimensions: many(behaviorDimension),
    rules: many(behaviorRule)
}));

export const behaviorDimensionRelations = relations(behaviorDimension, ({ one, many }) => ({
    matrix: one(behaviorMatrix, { fields: [behaviorDimension.matrixId], references: [behaviorMatrix.id] }),
    cells: many(behaviorCell)
}));

export const behaviorRuleRelations = relations(behaviorRule, ({ one, many }) => ({
    matrix: one(behaviorMatrix, { fields: [behaviorRule.matrixId], references: [behaviorMatrix.id] }),
    cells: many(behaviorCell)
}));

export const behaviorCellRelations = relations(behaviorCell, ({ one, many }) => ({
    rule: one(behaviorRule, { fields: [behaviorCell.ruleId], references: [behaviorRule.id] }),
    dimension: one(behaviorDimension, { fields: [behaviorCell.dimensionId], references: [behaviorDimension.id] }),
    cellRequirements: many(behaviorCellRequirement)
}));

export const behaviorCellRequirementRelations = relations(behaviorCellRequirement, ({ one }) => ({
    cell: one(behaviorCell, { fields: [behaviorCellRequirement.cellId], references: [behaviorCell.id] }),
    requirement: one(requirement, { fields: [behaviorCellRequirement.requirementId], references: [requirement.id] })
}));
