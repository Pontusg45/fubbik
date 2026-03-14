import {
    createConnection as createConnectionRepo,
    deleteConnection as deleteConnectionRepo,
    getChunkById,
    getConnectionById
} from "@fubbik/db/repository";
import { Effect } from "effect";

import { NotFoundError, ValidationError } from "../errors";

export function createConnection(userId: string, body: { sourceId: string; targetId: string; relation: string; origin?: string }) {
    return Effect.suspend(() => {
        if (body.sourceId === body.targetId) {
            return Effect.fail(new ValidationError({ message: "Cannot connect a chunk to itself" }));
        }
        return Effect.void;
    }).pipe(
        Effect.flatMap(() => getChunkById(body.sourceId, userId)),
        Effect.flatMap(source => (source ? Effect.succeed(source) : Effect.fail(new NotFoundError({ resource: "Source chunk" })))),
        Effect.flatMap(() => getChunkById(body.targetId, userId)),
        Effect.flatMap(target => (target ? Effect.succeed(target) : Effect.fail(new NotFoundError({ resource: "Target chunk" })))),
        Effect.flatMap(() => {
            const origin = body.origin ?? "human";
            return createConnectionRepo({
                id: crypto.randomUUID(),
                sourceId: body.sourceId,
                targetId: body.targetId,
                relation: body.relation,
                origin,
                reviewStatus: origin === "ai" ? "draft" : "approved"
            });
        })
    );
}

export function deleteConnection(connectionId: string, userId: string) {
    return getConnectionById(connectionId).pipe(
        Effect.flatMap(conn => (conn ? Effect.succeed(conn) : Effect.fail(new NotFoundError({ resource: "Connection" })))),
        Effect.flatMap(conn =>
            Effect.all({
                source: getChunkById(conn.sourceId, userId),
                target: getChunkById(conn.targetId, userId)
            }).pipe(
                Effect.flatMap(({ source, target }) =>
                    source || target ? Effect.succeed(conn) : Effect.fail(new NotFoundError({ resource: "Connection" }))
                )
            )
        ),
        Effect.flatMap(conn => deleteConnectionRepo(conn.id))
    );
}
