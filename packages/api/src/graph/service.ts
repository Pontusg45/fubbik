import { getAllChunksMeta, getAllConnectionsForUser } from "@fubbik/db/repository";
import { Effect } from "effect";

export function getUserGraph(userId?: string) {
    return Effect.all({ chunks: getAllChunksMeta(userId), connections: getAllConnectionsForUser(userId) }, { concurrency: "unbounded" });
}
