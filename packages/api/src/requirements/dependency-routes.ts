import { Effect } from "effect";
import { Elysia, t } from "elysia";
import { requireSession } from "../require-session";
import * as depService from "./dependency-service";

export const dependencyRoutes = new Elysia()
    .post(
        "/requirements/:id/dependencies",
        ctx =>
            Effect.runPromise(
                Effect.gen(function* () {
                    const session = yield* requireSession(ctx);
                    yield* depService.addDependency(ctx.params.id, ctx.body.dependsOnId, session.user.id);
                    ctx.set.status = 201;
                    return { message: "Dependency added" };
                })
            ),
        {
            body: t.Object({
                dependsOnId: t.String()
            })
        }
    )
    .delete(
        "/requirements/:id/dependencies/:dependsOnId",
        ctx =>
            Effect.runPromise(
                Effect.gen(function* () {
                    const session = yield* requireSession(ctx);
                    yield* depService.removeDependency(ctx.params.id, ctx.params.dependsOnId, session.user.id);
                    return { message: "Dependency removed" };
                })
            )
    )
    .get(
        "/requirements/:id/dependencies",
        ctx =>
            Effect.runPromise(
                Effect.gen(function* () {
                    const session = yield* requireSession(ctx);
                    return yield* depService.getDependencies(ctx.params.id, session.user.id);
                })
            )
    )
    .get(
        "/requirements/:id/dependencies/graph",
        ctx =>
            Effect.runPromise(
                Effect.gen(function* () {
                    const session = yield* requireSession(ctx);
                    return yield* depService.getDependencyGraph(ctx.params.id, session.user.id);
                })
            )
    );
