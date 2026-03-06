import { and, desc, eq, ilike, or, sql } from "drizzle-orm";
import { db } from "../index";
import { chunk, chunkConnection } from "../schema/chunk";

export interface ListChunksParams {
  userId: string;
  type?: string;
  search?: string;
  limit: number;
  offset: number;
}

export async function listChunks(params: ListChunksParams) {
  const conditions = [eq(chunk.userId, params.userId)];
  if (params.type) {
    conditions.push(eq(chunk.type, params.type));
  }
  if (params.search) {
    conditions.push(
      or(ilike(chunk.title, `%${params.search}%`), ilike(chunk.content, `%${params.search}%`))!,
    );
  }
  const chunks = await db
    .select()
    .from(chunk)
    .where(and(...conditions))
    .orderBy(desc(chunk.updatedAt))
    .limit(params.limit)
    .offset(params.offset);

  const total = await db
    .select({ count: sql<number>`count(*)` })
    .from(chunk)
    .where(and(...conditions));

  return { chunks, total: Number(total[0]?.count ?? 0) };
}

export async function getChunkById(chunkId: string, userId: string) {
  const [found] = await db
    .select()
    .from(chunk)
    .where(and(eq(chunk.id, chunkId), eq(chunk.userId, userId)));
  return found ?? null;
}

export async function getChunkConnections(chunkId: string) {
  return db
    .select({
      id: chunkConnection.id,
      targetId: chunkConnection.targetId,
      sourceId: chunkConnection.sourceId,
      relation: chunkConnection.relation,
      title: chunk.title,
    })
    .from(chunkConnection)
    .leftJoin(
      chunk,
      or(
        and(eq(chunkConnection.targetId, chunk.id), eq(chunkConnection.sourceId, chunkId)),
        and(eq(chunkConnection.sourceId, chunk.id), eq(chunkConnection.targetId, chunkId)),
      ),
    )
    .where(or(eq(chunkConnection.sourceId, chunkId), eq(chunkConnection.targetId, chunkId)));
}

export interface CreateChunkParams {
  id: string;
  title: string;
  content: string;
  type: string;
  tags: string[];
  userId: string;
}

export async function createChunk(params: CreateChunkParams) {
  const [created] = await db.insert(chunk).values(params).returning();
  return created;
}

export interface UpdateChunkParams {
  title?: string;
  content?: string;
  type?: string;
  tags?: string[];
}

export async function updateChunk(chunkId: string, params: UpdateChunkParams) {
  const [updated] = await db
    .update(chunk)
    .set({
      ...(params.title !== undefined && { title: params.title }),
      ...(params.content !== undefined && { content: params.content }),
      ...(params.type !== undefined && { type: params.type }),
      ...(params.tags !== undefined && { tags: params.tags }),
    })
    .where(eq(chunk.id, chunkId))
    .returning();
  return updated;
}

export async function deleteChunk(chunkId: string, userId: string) {
  const [deleted] = await db
    .delete(chunk)
    .where(and(eq(chunk.id, chunkId), eq(chunk.userId, userId)))
    .returning();
  return deleted ?? null;
}
