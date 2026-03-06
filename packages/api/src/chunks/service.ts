import {
  createChunk as createChunkRepo,
  deleteChunk as deleteChunkRepo,
  getChunkById,
  getChunkConnections,
  listChunks as listChunksRepo,
  updateChunk as updateChunkRepo,
} from "@fubbik/db/repository";

export async function listChunks(
  userId: string,
  query: { type?: string; search?: string; limit?: string; offset?: string },
) {
  const limit = Math.min(Number(query.limit ?? 50), 100);
  const offset = Number(query.offset ?? 0);
  const result = await listChunksRepo({ userId, type: query.type, search: query.search, limit, offset });
  return { ...result, limit, offset };
}

export async function getChunkDetail(chunkId: string, userId: string) {
  const found = await getChunkById(chunkId, userId);
  if (!found) return null;
  const connections = await getChunkConnections(chunkId);
  return { chunk: found, connections };
}

export async function createChunk(
  userId: string,
  body: { title: string; content?: string; type?: string; tags?: string[] },
) {
  return createChunkRepo({
    id: crypto.randomUUID(),
    title: body.title,
    content: body.content ?? "",
    type: body.type ?? "note",
    tags: body.tags ?? [],
    userId,
  });
}

export async function updateChunk(
  chunkId: string,
  userId: string,
  body: { title?: string; content?: string; type?: string; tags?: string[] },
) {
  const existing = await getChunkById(chunkId, userId);
  if (!existing) return null;
  return updateChunkRepo(chunkId, body);
}

export async function deleteChunk(chunkId: string, userId: string) {
  return deleteChunkRepo(chunkId, userId);
}
