import { and, count, desc, eq } from "drizzle-orm";
import { Effect } from "effect";

import { DatabaseError } from "../errors";
import { db } from "../index";
import { notification } from "../schema/notification";

export function listNotifications(userId: string, opts: { limit?: number; unreadOnly?: boolean } = {}) {
    return Effect.tryPromise({
        try: () => {
            const conditions = [eq(notification.userId, userId)];
            if (opts.unreadOnly) {
                conditions.push(eq(notification.read, false));
            }
            return db
                .select()
                .from(notification)
                .where(and(...conditions))
                .orderBy(desc(notification.createdAt))
                .limit(opts.limit ?? 50);
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function getUnreadCount(userId: string) {
    return Effect.tryPromise({
        try: async () => {
            const [result] = await db
                .select({ count: count() })
                .from(notification)
                .where(and(eq(notification.userId, userId), eq(notification.read, false)));
            return result?.count ?? 0;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function markAsRead(id: string, userId: string) {
    return Effect.tryPromise({
        try: async () => {
            const [updated] = await db
                .update(notification)
                .set({ read: true })
                .where(and(eq(notification.id, id), eq(notification.userId, userId)))
                .returning();
            return updated ?? null;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function markAllAsRead(userId: string) {
    return Effect.tryPromise({
        try: async () => {
            await db
                .update(notification)
                .set({ read: true })
                .where(and(eq(notification.userId, userId), eq(notification.read, false)));
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function createNotification(params: {
    id: string;
    userId: string;
    type: string;
    title: string;
    message: string;
    linkTo?: string;
}) {
    return Effect.tryPromise({
        try: async () => {
            const [created] = await db.insert(notification).values(params).returning();
            return created!;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function deleteNotification(id: string, userId: string) {
    return Effect.tryPromise({
        try: async () => {
            const [deleted] = await db
                .delete(notification)
                .where(and(eq(notification.id, id), eq(notification.userId, userId)))
                .returning();
            return deleted ?? null;
        },
        catch: cause => new DatabaseError({ cause })
    });
}
