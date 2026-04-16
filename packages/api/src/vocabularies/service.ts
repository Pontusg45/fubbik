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

export function getChunkTypes(userId: string, codebaseId?: string) {
    return listChunkTypes({ userId, codebaseId });
}

export function getConnectionRelations(userId: string, codebaseId?: string) {
    return listConnectionRelations({ userId, codebaseId });
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

type VocabError = ValidationError | NotFoundError;

export function createChunkType(userId: string, body: CreateChunkTypeBody) {
    return Effect.gen(function* () {
        if (!SLUG_RE.test(body.id)) {
            return yield* Effect.fail(
                new ValidationError({ message: "id must be a lowercase slug (letters, digits, - or _, max 41 chars)" })
            ) as Effect.Effect<never, VocabError>;
        }
        const existing = yield* findChunkTypeById(body.id);
        if (existing) {
            return yield* Effect.fail(
                new ValidationError({ message: `chunk type "${body.id}" already exists` })
            ) as Effect.Effect<never, VocabError>;
        }
        return yield* createChunkTypeRepo({ ...body, userId });
    });
}

export function updateChunkType(id: string, userId: string, body: Partial<Omit<CreateChunkTypeBody, "id">>) {
    return Effect.gen(function* () {
        const existing = yield* findChunkTypeById(id);
        if (!existing) {
            return yield* Effect.fail(new NotFoundError({ resource: "ChunkType" })) as Effect.Effect<never, VocabError>;
        }
        if (existing.builtIn) {
            return yield* Effect.fail(
                new ValidationError({ message: "builtin chunk types cannot be edited" })
            ) as Effect.Effect<never, VocabError>;
        }
        const updated = yield* updateChunkTypeRow(id, userId, body);
        if (!updated) {
            return yield* Effect.fail(new NotFoundError({ resource: "ChunkType" })) as Effect.Effect<never, VocabError>;
        }
        return updated;
    });
}

export function deleteChunkType(id: string, userId: string) {
    return Effect.gen(function* () {
        const existing = yield* findChunkTypeById(id);
        if (!existing) {
            return yield* Effect.fail(new NotFoundError({ resource: "ChunkType" })) as Effect.Effect<never, VocabError>;
        }
        if (existing.builtIn) {
            return yield* Effect.fail(
                new ValidationError({ message: "builtin chunk types cannot be deleted" })
            ) as Effect.Effect<never, VocabError>;
        }
        const deleted = yield* deleteChunkTypeRow(id, userId);
        if (!deleted) {
            return yield* Effect.fail(new NotFoundError({ resource: "ChunkType" })) as Effect.Effect<never, VocabError>;
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
            ) as Effect.Effect<never, VocabError>;
        }
        const existing = yield* findConnectionRelationById(body.id);
        if (existing) {
            return yield* Effect.fail(
                new ValidationError({ message: `relation "${body.id}" already exists` })
            ) as Effect.Effect<never, VocabError>;
        }
        return yield* createConnectionRelationRepo({ ...body, userId });
    });
}

export function updateConnectionRelation(id: string, userId: string, body: Partial<Omit<CreateRelationBody, "id">>) {
    return Effect.gen(function* () {
        const existing = yield* findConnectionRelationById(id);
        if (!existing) {
            return yield* Effect.fail(
                new NotFoundError({ resource: "ConnectionRelation" })
            ) as Effect.Effect<never, VocabError>;
        }
        if (existing.builtIn) {
            return yield* Effect.fail(
                new ValidationError({ message: "builtin relations cannot be edited" })
            ) as Effect.Effect<never, VocabError>;
        }
        const updated = yield* updateConnectionRelationRow(id, userId, body);
        if (!updated) {
            return yield* Effect.fail(
                new NotFoundError({ resource: "ConnectionRelation" })
            ) as Effect.Effect<never, VocabError>;
        }
        return updated;
    });
}

export function deleteConnectionRelation(id: string, userId: string) {
    return Effect.gen(function* () {
        const existing = yield* findConnectionRelationById(id);
        if (!existing) {
            return yield* Effect.fail(
                new NotFoundError({ resource: "ConnectionRelation" })
            ) as Effect.Effect<never, VocabError>;
        }
        if (existing.builtIn) {
            return yield* Effect.fail(
                new ValidationError({ message: "builtin relations cannot be deleted" })
            ) as Effect.Effect<never, VocabError>;
        }
        const deleted = yield* deleteConnectionRelationRow(id, userId);
        if (!deleted) {
            return yield* Effect.fail(
                new NotFoundError({ resource: "ConnectionRelation" })
            ) as Effect.Effect<never, VocabError>;
        }
        return deleted;
    });
}
