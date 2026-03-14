import { Effect } from "effect";
import { Elysia, t } from "elysia";

import { requireSession } from "../require-session";
import * as notificationService from "./service";

export const notificationRoutes = new Elysia()
    .get(
        "/notifications/count",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session => notificationService.getUnreadCount(session.user.id)),
                    Effect.map(count => ({ count }))
                )
            )
    )
    .post("/notifications/read-all", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session => notificationService.markAllAsRead(session.user.id)),
                Effect.map(() => ({ message: "All marked as read" }))
            )
        )
    )
    .get(
        "/notifications",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        notificationService.listNotifications(session.user.id, {
                            limit: ctx.query.limit ? Number(ctx.query.limit) : undefined,
                            unreadOnly: ctx.query.unreadOnly === "true"
                        })
                    )
                )
            ),
        {
            query: t.Object({
                limit: t.Optional(t.String()),
                unreadOnly: t.Optional(t.String())
            })
        }
    )
    .patch("/notifications/:id/read", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session => notificationService.markAsRead(ctx.params.id, session.user.id))
            )
        )
    )
    .delete("/notifications/:id", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session => notificationService.deleteNotification(ctx.params.id, session.user.id)),
                Effect.map(() => ({ message: "Deleted" }))
            )
        )
    );
