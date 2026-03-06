import { getChunkCount, getConnectionCount, getTagCount } from "@fubbik/db/repository";

export async function getUserStats(userId: string) {
  const [chunks, connections, tags] = await Promise.all([
    getChunkCount(userId),
    getConnectionCount(userId),
    getTagCount(userId),
  ]);
  return { chunks, connections, tags };
}
