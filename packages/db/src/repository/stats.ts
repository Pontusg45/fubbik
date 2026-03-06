import { eq, sql } from "drizzle-orm";
import { db } from "../index";
import { chunk, chunkConnection } from "../schema/chunk";

export async function getChunkCount(userId: string) {
  const [result] = await db
    .select({ count: sql<number>`count(*)` })
    .from(chunk)
    .where(eq(chunk.userId, userId));
  return Number(result?.count ?? 0);
}

export async function getConnectionCount(userId: string) {
  const [result] = await db
    .select({ count: sql<number>`count(*)` })
    .from(chunkConnection)
    .innerJoin(chunk, eq(chunkConnection.sourceId, chunk.id))
    .where(eq(chunk.userId, userId));
  return Number(result?.count ?? 0);
}

export async function getTagCount(userId: string) {
  const [result] = await db
    .select({
      count: sql<number>`count(distinct tag)`,
    })
    .from(
      sql`(select jsonb_array_elements_text(${chunk.tags}) as tag from ${chunk} where ${chunk.userId} = ${userId}) t`,
    );
  return Number(result?.count ?? 0);
}
