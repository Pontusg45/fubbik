import { and, gte, lte } from "drizzle-orm";

import { db, dbEffect } from "../index";
import { graphEvent } from "../schema/graph-event";

export function insertGraphEvent(data: {
    id: string;
    vertexLabel: string;
    vertexId: string;
    action: string;
    edgeType?: string;
    edgeTargetId?: string;
    snapshot?: Record<string, unknown>;
}) {
    return dbEffect(async () => {
        await db.insert(graphEvent).values(data);
    });
}

export function getGraphEventsUpTo(before: Date, limit = 10000) {
    return dbEffect(async () => {
        return db.select().from(graphEvent).where(lte(graphEvent.createdAt, before)).orderBy(graphEvent.createdAt).limit(limit);
    });
}

export function getGraphEventsBetween(from: Date, to: Date) {
    return dbEffect(async () => {
        return db
            .select()
            .from(graphEvent)
            .where(and(gte(graphEvent.createdAt, from), lte(graphEvent.createdAt, to)))
            .orderBy(graphEvent.createdAt);
    });
}
