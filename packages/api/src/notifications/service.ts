import {
    createNotification as createNotificationRepo,
    deleteNotification as deleteNotificationRepo,
    getUnreadCount as getUnreadCountRepo,
    listNotifications as listNotificationsRepo,
    markAllAsRead as markAllAsReadRepo,
    markAsRead as markAsReadRepo
} from "@fubbik/db/repository";
import { Effect } from "effect";

import { NotFoundError } from "../errors";

export function listNotifications(userId: string, opts: { limit?: number; unreadOnly?: boolean } = {}) {
    return listNotificationsRepo(userId, opts);
}

export function getUnreadCount(userId: string) {
    return getUnreadCountRepo(userId);
}

export function markAsRead(id: string, userId: string) {
    return markAsReadRepo(id, userId).pipe(
        Effect.flatMap(n => (n ? Effect.succeed(n) : Effect.fail(new NotFoundError({ resource: "Notification" }))))
    );
}

export function markAllAsRead(userId: string) {
    return markAllAsReadRepo(userId);
}

export function createNotification(params: {
    userId: string;
    type: string;
    title: string;
    message: string;
    linkTo?: string;
}) {
    return createNotificationRepo({
        id: crypto.randomUUID(),
        ...params
    });
}

export function deleteNotification(id: string, userId: string) {
    return deleteNotificationRepo(id, userId).pipe(
        Effect.flatMap(n => (n ? Effect.succeed(n) : Effect.fail(new NotFoundError({ resource: "Notification" }))))
    );
}
