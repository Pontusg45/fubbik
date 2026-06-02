import { relations, sql } from "drizzle-orm";
import { jsonb, pgTable, text, uniqueIndex } from "drizzle-orm/pg-core";

import { space } from "./space";

export const spaceCodeMetadata = pgTable(
    "space_code_metadata",
    {
        spaceId: text("space_id")
            .primaryKey()
            .references(() => space.id, { onDelete: "cascade" }),
        userId: text("user_id").notNull(),
        remoteUrl: text("remote_url"),
        localPaths: jsonb("local_paths").$type<string[]>().notNull().default([])
    },
    table => [
        uniqueIndex("space_code_user_remote_idx")
            .on(table.userId, table.remoteUrl)
            .where(sql`"remote_url" IS NOT NULL`)
    ]
);

export const spaceCodeMetadataRelations = relations(spaceCodeMetadata, ({ one }) => ({
    space: one(space, { fields: [spaceCodeMetadata.spaceId], references: [space.id] })
}));
