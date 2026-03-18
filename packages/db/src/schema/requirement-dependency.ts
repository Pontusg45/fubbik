import { pgTable, primaryKey, text, check } from "drizzle-orm/pg-core";
import { sql } from "drizzle-orm";
import { requirement } from "./requirement";

export const requirementDependency = pgTable(
    "requirement_dependency",
    {
        requirementId: text("requirement_id")
            .notNull()
            .references(() => requirement.id, { onDelete: "cascade" }),
        dependsOnId: text("depends_on_id")
            .notNull()
            .references(() => requirement.id, { onDelete: "cascade" })
    },
    table => [
        primaryKey({ columns: [table.requirementId, table.dependsOnId] }),
        check("no_self_dependency", sql`${table.requirementId} != ${table.dependsOnId}`)
    ]
);
