/**
 * Factory helpers that return ready-to-insert row objects with sensible defaults.
 *
 * Use these instead of spelling out every column every time. Callers pass
 * `userId` + a small set of semantic fields; factories fill defaults (ids,
 * timestamps, flags). Overrides win over defaults.
 *
 *   const c = makeChunk({ userId, title: "Why Effect", type: "convention" });
 *   // → { id: "...", title: ..., userId, type: "convention", content: "", ... }
 */

import type { chunk, chunkConnection } from "../schema/chunk";
import type { codebase } from "../schema/codebase";
import type { tag, tagType } from "../schema/tag";

type NewChunk = typeof chunk.$inferInsert;
type NewChunkConnection = typeof chunkConnection.$inferInsert;
type NewTagType = typeof tagType.$inferInsert;
type NewTag = typeof tag.$inferInsert;
type NewCodebase = typeof codebase.$inferInsert;

export function uuid(): string {
    return crypto.randomUUID();
}

/** Makes a NewChunk with default content, reviewStatus, origin, etc. */
export function makeChunk(input: {
    id?: string;
    userId: string;
    title: string;
    type?: string;
    content?: string;
    summary?: string | null;
    rationale?: string | null;
    consequences?: string | null;
}): NewChunk {
    return {
        id: input.id ?? uuid(),
        userId: input.userId,
        title: input.title,
        type: input.type ?? "note",
        content: input.content ?? "",
        summary: input.summary ?? null,
        rationale: input.rationale ?? null,
        consequences: input.consequences ?? null,
        aliases: [],
        notAbout: [],
        scope: {},
        origin: "human",
        reviewStatus: "approved"
    };
}

/** Builds a NewChunkConnection given both endpoint IDs + a relation slug. */
export function makeConnection(
    sourceId: string,
    targetId: string,
    relation: string
): NewChunkConnection {
    return {
        id: uuid(),
        sourceId,
        targetId,
        relation,
        origin: "human",
        reviewStatus: "approved"
    };
}

export function makeTagType(input: { id?: string; name: string; color?: string; userId: string }): NewTagType {
    return {
        id: input.id ?? uuid(),
        name: input.name,
        color: input.color ?? "#8b5cf6",
        userId: input.userId
    };
}

export function makeTag(input: {
    id?: string;
    name: string;
    tagTypeId?: string | null;
    userId: string;
}): NewTag {
    return {
        id: input.id ?? uuid(),
        name: input.name,
        tagTypeId: input.tagTypeId ?? null,
        userId: input.userId,
        origin: "human",
        reviewStatus: "approved"
    };
}

export function makeCodebase(input: {
    id?: string;
    userId: string;
    name: string;
    remoteUrl?: string | null;
    localPaths?: string[];
}): NewCodebase {
    return {
        id: input.id ?? uuid(),
        userId: input.userId,
        name: input.name,
        remoteUrl: input.remoteUrl ?? null,
        localPaths: input.localPaths ?? []
    };
}
