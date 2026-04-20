import { and, count, desc, eq } from "drizzle-orm";

import { db, dbEffect } from "../index";
import { notification } from "../schema/notification";

export function listNotifications(userId: string, opts: { limit?: number; unreadOnly?: boolean } = {}) {
    return dbEffect(() => {
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
        });
}

export function getUnreadCount(userId: string) {
    return dbEffect(async () => {
            const [result] = await db
                .select({ count: count() })
                .from(notification)
                .where(and(eq(notification.userId, userId), eq(notification.read, false)));
            return result?.count ?? 0;
        });
}

export function markAsRead(id: string, userId: string) {
    return dbEffect(async () => {
            const [updated] = await db
                .update(notification)
                .set({ read: true })
                .where(and(eq(notification.id, id), eq(notification.userId, userId)))
                .returning();
            return updated ?? null;
        });
}

export function markAllAsRead(userId: string) {
    return dbEffect(async () => {
            await db
                .update(notification)
                .set({ read: true })
                .where(and(eq(notification.userId, userId), eq(notification.read, false)));
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
    return dbEffect(async () => {
            const [created] = await db.insert(notification).values(params).returning();
            return created!;
        });
}

export function deleteNotification(id: string, userId: string) {
    return dbEffect(async () => {
            const [deleted] = await db
                .delete(notification)
                .where(and(eq(notification.id, id), eq(notification.userId, userId)))
                .returning();
            return deleted ?? null;
        });
}
