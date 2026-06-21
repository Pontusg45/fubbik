import { relations } from "drizzle-orm";
import { index, integer, jsonb, pgTable, primaryKey, text, timestamp, unique } from "drizzle-orm/pg-core";

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
        // Decision context — the "why" behind a behavior (mirrors chunk rationale fields).
        rationale: text("rationale"),
        alternatives: text("alternatives"),
        consequences: text("consequences"),
        // Explicit negative space: what violating this behavior looks like.
        counterexample: text("counterexample"),
        order: integer("order").notNull().default(0),
        createdAt: timestamp("created_at").defaultNow().notNull(),
        updatedAt: timestamp("updated_at")
            .defaultNow()
            .$onUpdate(() => new Date())
            .notNull()
    },
    table => [index("behavior_rule_matrixId_idx").on(table.matrixId)]
);

/**
 * Append-only history of behavior rule edits. A new row is written on every
 * mutation so the evolution of an invariant/contract is auditable over time
 * (and surfaceable in the graph timeline).
 */
export interface BehaviorRuleSnapshot {
    title: string;
    description: string | null;
    category: string | null;
    rationale: string | null;
    alternatives: string | null;
    consequences: string | null;
    counterexample: string | null;
}

export const behaviorRuleVersion = pgTable(
    "behavior_rule_version",
    {
        id: text("id").primaryKey(),
        ruleId: text("rule_id")
            .notNull()
            .references(() => behaviorRule.id, { onDelete: "cascade" }),
        // Plain jsonb (not $type-branded) so the inferred row type stays portable
        // through Eden treaty inference. The write side is typed via insertRuleVersion.
        snapshot: jsonb("snapshot").notNull(),
        changedBy: text("changed_by").references(() => user.id, { onDelete: "set null" }),
        createdAt: timestamp("created_at").defaultNow().notNull()
    },
    table => [index("behavior_rule_version_ruleId_idx").on(table.ruleId)]
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

/**
 * Links a behavior cell to the concrete code that implements or verifies it.
 * `kind` discriminates between source files, exported symbols, and tests.
 * `ref` is a file path, a "path::symbol" identifier, or a test path/name.
 */
export const behaviorCellCode = pgTable(
    "behavior_cell_code",
    {
        id: text("id").primaryKey(),
        cellId: text("cell_id")
            .notNull()
            .references(() => behaviorCell.id, { onDelete: "cascade" }),
        kind: text("kind").notNull(), // 'file' | 'symbol' | 'test'
        ref: text("ref").notNull(),
        createdAt: timestamp("created_at").defaultNow().notNull()
    },
    table => [
        unique("behavior_cell_code_cell_kind_ref").on(table.cellId, table.kind, table.ref),
        index("behavior_cell_code_cellId_idx").on(table.cellId),
        index("behavior_cell_code_ref_idx").on(table.ref)
    ]
);

/**
 * Recorded outcomes of tests that verify a behavior cell. Drives the
 * "verified"/"violated" cell status from real test runs rather than a
 * manually-toggled badge.
 */
export const behaviorTestResult = pgTable(
    "behavior_test_result",
    {
        id: text("id").primaryKey(),
        cellId: text("cell_id")
            .notNull()
            .references(() => behaviorCell.id, { onDelete: "cascade" }),
        testRef: text("test_ref").notNull(),
        status: text("status").notNull(), // 'pass' | 'fail'
        detail: text("detail"),
        runAt: timestamp("run_at").defaultNow().notNull()
    },
    table => [index("behavior_test_result_cellId_idx").on(table.cellId)]
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
    cells: many(behaviorCell),
    versions: many(behaviorRuleVersion)
}));

export const behaviorRuleVersionRelations = relations(behaviorRuleVersion, ({ one }) => ({
    rule: one(behaviorRule, { fields: [behaviorRuleVersion.ruleId], references: [behaviorRule.id] }),
    changedByUser: one(user, { fields: [behaviorRuleVersion.changedBy], references: [user.id] })
}));

export const behaviorCellRelations = relations(behaviorCell, ({ one, many }) => ({
    rule: one(behaviorRule, { fields: [behaviorCell.ruleId], references: [behaviorRule.id] }),
    dimension: one(behaviorDimension, { fields: [behaviorCell.dimensionId], references: [behaviorDimension.id] }),
    cellRequirements: many(behaviorCellRequirement),
    codeLinks: many(behaviorCellCode),
    testResults: many(behaviorTestResult)
}));

export const behaviorCellCodeRelations = relations(behaviorCellCode, ({ one }) => ({
    cell: one(behaviorCell, { fields: [behaviorCellCode.cellId], references: [behaviorCell.id] })
}));

export const behaviorTestResultRelations = relations(behaviorTestResult, ({ one }) => ({
    cell: one(behaviorCell, { fields: [behaviorTestResult.cellId], references: [behaviorCell.id] })
}));

export const behaviorCellRequirementRelations = relations(behaviorCellRequirement, ({ one }) => ({
    cell: one(behaviorCell, { fields: [behaviorCellRequirement.cellId], references: [behaviorCell.id] }),
    requirement: one(requirement, { fields: [behaviorCellRequirement.requirementId], references: [requirement.id] })
}));
