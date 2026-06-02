import { relations } from "drizzle-orm";
import { boolean, integer, pgTable, text, timestamp } from "drizzle-orm/pg-core";

import { space } from "./space";

export const spaceKind = pgTable("space_kind", {
    id: text("id").primaryKey(),
    label: text("label").notNull(),
    description: text("description"),
    icon: text("icon"),
    displayOrder: integer("display_order").notNull().default(100),
    builtIn: boolean("built_in").notNull().default(false),
    createdAt: timestamp("created_at").defaultNow().notNull(),
    updatedAt: timestamp("updated_at")
        .defaultNow()
        .$onUpdate(() => new Date())
        .notNull()
});

export const spaceKindRelations = relations(spaceKind, ({ many }) => ({
    spaces: many(space)
}));
