import { index, jsonb, pgTable, text, timestamp } from "drizzle-orm/pg-core";

export const graphEvent = pgTable(
    "graph_event",
    {
        id: text("id").primaryKey(),
        vertexLabel: text("vertex_label").notNull(),
        vertexId: text("vertex_id").notNull(),
        edgeType: text("edge_type"),
        edgeTargetId: text("edge_target_id"),
        action: text("action").notNull(), // "created" | "updated" | "deleted" | "property_changed"
        snapshot: jsonb("snapshot").$type<Record<string, unknown>>(),
        createdAt: timestamp("created_at").defaultNow().notNull()
    },
    table => [
        index("graph_event_vertexId_idx").on(table.vertexId),
        index("graph_event_createdAt_idx").on(table.createdAt),
        index("graph_event_vertexLabel_idx").on(table.vertexLabel)
    ]
);

export type GraphEvent = typeof graphEvent.$inferSelect;
export type NewGraphEvent = typeof graphEvent.$inferInsert;
