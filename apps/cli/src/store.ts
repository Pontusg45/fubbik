import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

export interface Chunk {
  id: string;
  title: string;
  content: string;
  type: string;
  tags: string[];
  createdAt: string;
  updatedAt: string;
}

interface Store {
  name: string;
  chunks: Chunk[];
}

const STORE_DIR = ".fubbik";
const STORE_FILE = "store.json";

function storePath(dir: string = process.cwd()): string {
  return join(dir, STORE_DIR, STORE_FILE);
}

export function storeDir(dir: string = process.cwd()): string {
  return join(dir, STORE_DIR);
}

export function storeExists(dir?: string): boolean {
  return existsSync(storePath(dir));
}

export function createStore(name: string, dir?: string): Store {
  const base = dir ?? process.cwd();
  const storeDirectory = join(base, STORE_DIR);
  mkdirSync(storeDirectory, { recursive: true });
  const store: Store = { name, chunks: [] };
  writeFileSync(storePath(base), JSON.stringify(store, null, 2));
  return store;
}

export function readStore(dir?: string): Store {
  const path = storePath(dir);
  if (!existsSync(path)) {
    throw new Error(`No knowledge base found. Run "fubbik init" first.`);
  }
  return JSON.parse(readFileSync(path, "utf-8")) as Store;
}

function saveStore(store: Store, dir?: string): void {
  writeFileSync(storePath(dir), JSON.stringify(store, null, 2));
}

function generateId(): string {
  return `c-${Date.now().toString(36)}`;
}

export function addChunk(
  data: { title: string; content: string; type: string; tags: string[] },
  dir?: string,
): Chunk {
  const store = readStore(dir);
  const now = new Date().toISOString();
  const chunk: Chunk = {
    id: generateId(),
    title: data.title,
    content: data.content,
    type: data.type,
    tags: data.tags,
    createdAt: now,
    updatedAt: now,
  };
  store.chunks.push(chunk);
  saveStore(store, dir);
  return chunk;
}

export function getChunk(id: string, dir?: string): Chunk | undefined {
  const store = readStore(dir);
  return store.chunks.find((c) => c.id === id);
}

export function listChunks(opts: { type?: string; tag?: string } = {}, dir?: string): Chunk[] {
  const store = readStore(dir);
  let chunks = store.chunks;
  if (opts.type) {
    chunks = chunks.filter((c) => c.type === opts.type);
  }
  if (opts.tag) {
    chunks = chunks.filter((c) => c.tags.includes(opts.tag!));
  }
  return chunks;
}

export function searchChunks(query: string, dir?: string): Chunk[] {
  const store = readStore(dir);
  const q = query.toLowerCase();
  return store.chunks.filter(
    (c) =>
      c.title.toLowerCase().includes(q) ||
      c.content.toLowerCase().includes(q) ||
      c.tags.some((t) => t.toLowerCase().includes(q)),
  );
}

export function deleteChunk(id: string, dir?: string): boolean {
  const store = readStore(dir);
  const idx = store.chunks.findIndex((c) => c.id === id);
  if (idx === -1) return false;
  store.chunks.splice(idx, 1);
  saveStore(store, dir);
  return true;
}

export function updateChunk(
  id: string,
  updates: Partial<Pick<Chunk, "title" | "content" | "type" | "tags">>,
  dir?: string,
): Chunk | undefined {
  const store = readStore(dir);
  const chunk = store.chunks.find((c) => c.id === id);
  if (!chunk) return undefined;
  if (updates.title !== undefined) chunk.title = updates.title;
  if (updates.content !== undefined) chunk.content = updates.content;
  if (updates.type !== undefined) chunk.type = updates.type;
  if (updates.tags !== undefined) chunk.tags = updates.tags;
  chunk.updatedAt = new Date().toISOString();
  saveStore(store, dir);
  return chunk;
}
