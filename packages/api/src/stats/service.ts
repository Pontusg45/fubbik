import { Effect } from "effect";
import { getChunkCount, getConnectionCount, getTagCount } from "@fubbik/db/repository";

export function getUserStats(userId: string) {
  return Effect.all(
    {
      chunks: getChunkCount(userId),
      connections: getConnectionCount(userId),
      tags: getTagCount(userId),
    },
    { concurrency: "unbounded" },
  );
}
