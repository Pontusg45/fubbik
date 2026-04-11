import { Effect } from "effect";
import { Elysia } from "elysia";
import { requireSession } from "../require-session";
import { computeClusters } from "./clusters";

export const clusterRoutes = new Elysia().get(
    "/chunks/clusters",
    ctx =>
        Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session =>
                    computeClusters(session.user.id).pipe(
                        Effect.orElse(() => Effect.succeed([]))
                    )
                )
            )
        )
);
