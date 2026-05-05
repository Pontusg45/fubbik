import { jsonb, pgTable, text, timestamp, unique } from "drizzle-orm/pg-core";

import { user } from "./auth";

export const scopeKey = pgTable(
    "scope_key",
    {
        id: text("id").primaryKey(),
        userId: text("user_id")
            .notNull()
            .references(() => user.id, { onDelete: "cascade" }),
        key: text("key").notNull(),
        description: text("description"),
        valueType: text("value_type").notNull().default("string"),
        allowedValues: jsonb("allowed_values"),
        createdAt: timestamp("created_at").defaultNow().notNull()
    },
    table => [unique("scope_key_user_key_unique").on(table.userId, table.key)]
);
