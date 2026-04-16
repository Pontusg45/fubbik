import { relations } from "drizzle-orm";
import { boolean, index, integer, pgTable, text, timestamp, type AnyPgColumn } from "drizzle-orm/pg-core";

import { user } from "./auth";
import { codebase } from "./codebase";

/**
 * Catalog of connection relations (the labels on chunk-to-chunk edges).
 *
 * Builtin rows are seeded (builtIn = true). Users / codebases can register custom
 * relations. `chunk_connection.relation` is still text — FK cutover happens later.
 *
 * `inverseOfId` lets pairs like `depends_on` ↔ `required_by` know about each other
 * so the graph can show both directions on a single edge.
 */
export const connectionRelation = pgTable(
    "connection_relation",
    {
        id: text("id").primaryKey(), // slug — e.g. "depends_on"
        label: text("label").notNull(), // e.g. "Depends on"
        description: text("description"),
        arrowStyle: text("arrow_style").notNull().default("solid"), // "solid" | "dashed" | "dotted"
        direction: text("direction").notNull().default("forward"), // "forward" | "bidirectional"
        color: text("color").notNull().default("#64748b"),
        inverseOfId: text("inverse_of_id").references((): AnyPgColumn => connectionRelation.id, { onDelete: "set null" }),
        displayOrder: integer("display_order").notNull().default(100),
        builtIn: boolean("built_in").notNull().default(false),
        userId: text("user_id").references(() => user.id, { onDelete: "cascade" }),
        codebaseId: text("codebase_id").references(() => codebase.id, { onDelete: "cascade" }),
        createdAt: timestamp("created_at").defaultNow().notNull(),
        updatedAt: timestamp("updated_at")
            .defaultNow()
            .$onUpdate(() => new Date())
            .notNull()
    },
    table => [
        index("connection_relation_userId_idx").on(table.userId),
        index("connection_relation_codebaseId_idx").on(table.codebaseId)
    ]
);

export const connectionRelationRelations = relations(connectionRelation, ({ one }) => ({
    user: one(user, { fields: [connectionRelation.userId], references: [user.id] }),
    codebase: one(codebase, { fields: [connectionRelation.codebaseId], references: [codebase.id] }),
    inverseOf: one(connectionRelation, {
        fields: [connectionRelation.inverseOfId],
        references: [connectionRelation.id],
        relationName: "relation_inverse"
    })
}));
