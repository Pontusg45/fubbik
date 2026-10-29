import {
    createChunkType as createChunkTypeRepo,
    createConnectionRelation as createConnectionRelationRepo,
    deleteChunkTypeRow,
    deleteConnectionRelationRow,
    findChunkTypeById,
    findConnectionRelationById,
    listChunkTypes,
    listConnectionRelations,
    updateChunkTypeRow,
    updateConnectionRelationRow
} from "@fubbik/db/repository";
import { Effect } from "effect";

import { NotFoundError, ValidationError } from "../errors";

export function getChunkTypes(userId: string, spaceId?: string) {
    return listChunkTypes({ userId, spaceId });
}

export function getConnectionRelations(userId: string, spaceId?: string) {
    return listConnectionRelations({ userId, spaceId });
}

// --- chunk_type mutations ---------------------------------------------------

export interface CreateChunkTypeBody {
    id: string;
    label: string;
    description?: string | null;
    icon?: string | null;
    color?: string;
    examples?: string[];
    displayOrder?: number;
    codebaseId?: string | null;
}

const SLUG_RE = /^[a-z0-9][a-z0-9_-]{0,40}$/;

export function createChunkType(userId: string, body: CreateChunkTypeBody) {
    return Effect.gen(function* () {
        if (!SLUG_RE.test(body.id)) {
            return yield* Effect.fail(
                new ValidationError({ message: "id must be a lowercase slug (letters, digits, - or _, max 41 chars)" })
            );
        }
        const existing = yield* findChunkTypeById(body.id);
        if (existing) {
            return yield* Effect.fail(new ValidationError({ message: `chunk type "${body.id}" already exists` }));
        }
        return yield* createChunkTypeRepo({ ...body, userId });
    });
}

export function updateChunkType(id: string, userId: string, body: Partial<Omit<CreateChunkTypeBody, "id">>) {
    return Effect.gen(function* () {
        const existing = yield* findChunkTypeById(id);
        if (!existing) {
            return yield* Effect.fail(new NotFoundError({ resource: "ChunkType" }));
        }
        if (existing.builtIn) {
            return yield* Effect.fail(new ValidationError({ message: "builtin chunk types cannot be edited" }));
        }
        const updated = yield* updateChunkTypeRow(id, userId, body);
        if (!updated) {
            return yield* Effect.fail(new NotFoundError({ resource: "ChunkType" }));
        }
        return updated;
    });
}

export function deleteChunkType(id: string, userId: string) {
    return Effect.gen(function* () {
        const existing = yield* findChunkTypeById(id);
        if (!existing) {
            return yield* Effect.fail(new NotFoundError({ resource: "ChunkType" }));
        }
        if (existing.builtIn) {
            return yield* Effect.fail(new ValidationError({ message: "builtin chunk types cannot be deleted" }));
        }
        const deleted = yield* deleteChunkTypeRow(id, userId);
        if (!deleted) {
            return yield* Effect.fail(new NotFoundError({ resource: "ChunkType" }));
        }
        return deleted;
    });
}

// --- connection_relation mutations -----------------------------------------

export interface CreateRelationBody {
    id: string;
    label: string;
    description?: string | null;
    arrowStyle?: "solid" | "dashed" | "dotted";
    direction?: "forward" | "bidirectional";
    color?: string;
    inverseOfId?: string | null;
    displayOrder?: number;
    codebaseId?: string | null;
}

export function createConnectionRelation(userId: string, body: CreateRelationBody) {
    return Effect.gen(function* () {
        if (!SLUG_RE.test(body.id)) {
            return yield* Effect.fail(
                new ValidationError({ message: "id must be a lowercase slug (letters, digits, - or _, max 41 chars)" })
            );
        }
        const existing = yield* findConnectionRelationById(body.id);
        if (existing) {
            return yield* Effect.fail(new ValidationError({ message: `relation "${body.id}" already exists` }));
        }
        return yield* createConnectionRelationRepo({ ...body, userId });
    });
}

export function updateConnectionRelation(id: string, userId: string, body: Partial<Omit<CreateRelationBody, "id">>) {
    return Effect.gen(function* () {
        const existing = yield* findConnectionRelationById(id);
        if (!existing) {
            return yield* Effect.fail(new NotFoundError({ resource: "ConnectionRelation" }));
        }
        if (existing.builtIn) {
            return yield* Effect.fail(new ValidationError({ message: "builtin relations cannot be edited" }));
        }
        const updated = yield* updateConnectionRelationRow(id, userId, body);
        if (!updated) {
            return yield* Effect.fail(new NotFoundError({ resource: "ConnectionRelation" }));
        }
        return updated;
    });
}

export function deleteConnectionRelation(id: string, userId: string) {
    return Effect.gen(function* () {
        const existing = yield* findConnectionRelationById(id);
        if (!existing) {
            return yield* Effect.fail(new NotFoundError({ resource: "ConnectionRelation" }));
        }
        if (existing.builtIn) {
            return yield* Effect.fail(new ValidationError({ message: "builtin relations cannot be deleted" }));
        }
        const deleted = yield* deleteConnectionRelationRow(id, userId);
        if (!deleted) {
            return yield* Effect.fail(new NotFoundError({ resource: "ConnectionRelation" }));
        }
        return deleted;
    });
}
