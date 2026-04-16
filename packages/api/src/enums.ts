/**
 * Single source of truth for chunk types and connection relations.
 * Both server (packages/api) and client (apps/web via `@fubbik/api`) import from here.
 * Adding a type or relation should be a one-line change.
 */

export const CHUNK_TYPES = [
    "note",
    "document",
    "guide",
    "reference",
    "schema",
    "checklist",
    "convention"
] as const;

export type ChunkType = (typeof CHUNK_TYPES)[number];

export const CONNECTION_RELATIONS = [
    "related_to",
    "part_of",
    "depends_on",
    "extends",
    "references",
    "supports",
    "contradicts",
    "alternative_to"
] as const;

export type ConnectionRelation = (typeof CONNECTION_RELATIONS)[number];

export function isChunkType(value: string): value is ChunkType {
    return (CHUNK_TYPES as readonly string[]).includes(value);
}

export function isConnectionRelation(value: string): value is ConnectionRelation {
    return (CONNECTION_RELATIONS as readonly string[]).includes(value);
}
