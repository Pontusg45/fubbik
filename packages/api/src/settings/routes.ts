import { Effect } from "effect";
import { Elysia, t } from "elysia";

import { requireSession } from "../require-session";
import * as settingsService from "./service";

export const settingsRoutes = new Elysia()
    .get("/settings/features", () =>
        Effect.runPromise(settingsService.getFeatureFlags())
    )
    .get("/settings/user", ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session => settingsService.getAllUserSettings(session.user.id))
            )
        )
    )
    .patch(
        "/settings/user",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(session =>
                        settingsService.setUserSetting(session.user.id, ctx.body.key, ctx.body.value)
                    ),
                    Effect.map(() => ({ message: "Updated" }))
                )
            ),
        {
            body: t.Object({
                key: t.String(),
                value: t.Unknown()
            })
        }
    )
    .get(
        "/settings/codebase",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(() =>
                        settingsService.getAllCodebaseSettings(ctx.query.codebaseId)
                    )
                )
            ),
        {
            query: t.Object({
                codebaseId: t.String()
            })
        }
    )
    .patch(
        "/settings/codebase",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(() =>
                        settingsService.setCodebaseSetting(ctx.body.codebaseId, ctx.body.key, ctx.body.value)
                    ),
                    Effect.map(() => ({ message: "Updated" }))
                )
            ),
        {
            body: t.Object({
                codebaseId: t.String(),
                key: t.String(),
                value: t.Unknown()
            })
        }
    )
    .get("/settings/instance", () =>
        Effect.runPromise(settingsService.getAllInstanceSettings())
    )
    .patch(
        "/settings/instance",
        ctx =>
            Effect.runPromise(
                requireSession(ctx).pipe(
                    Effect.flatMap(() =>
                        settingsService.setInstanceSetting(ctx.body.key, ctx.body.value)
                    ),
                    Effect.map(() => ({ message: "Updated" }))
                )
            ),
        {
            body: t.Object({
                key: t.String(),
                value: t.Unknown()
            })
        }
    );
