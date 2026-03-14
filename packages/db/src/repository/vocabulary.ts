import { asc, eq, sql } from "drizzle-orm";
import { Effect } from "effect";

import { DatabaseError } from "../errors";
import { db } from "../index";
import { vocabularyEntry } from "../schema/vocabulary";

export function listVocabulary(codebaseId: string) {
    return Effect.tryPromise({
        try: () =>
            db
                .select()
                .from(vocabularyEntry)
                .where(eq(vocabularyEntry.codebaseId, codebaseId))
                .orderBy(asc(vocabularyEntry.category), asc(vocabularyEntry.word)),
        catch: cause => new DatabaseError({ cause })
    });
}

export interface CreateVocabularyEntryParams {
    id: string;
    word: string;
    category: string;
    expects?: string[];
    codebaseId: string;
    userId: string;
}

export function createVocabularyEntry(params: CreateVocabularyEntryParams) {
    return Effect.tryPromise({
        try: async () => {
            const [created] = await db
                .insert(vocabularyEntry)
                .values({
                    id: params.id,
                    word: params.word.toLowerCase(),
                    category: params.category,
                    expects: params.expects ?? null,
                    codebaseId: params.codebaseId,
                    userId: params.userId
                })
                .returning();
            return created!;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function createVocabularyEntries(
    entries: Array<{
        id: string;
        word: string;
        category: string;
        expects?: string[];
        codebaseId: string;
        userId: string;
    }>
) {
    if (entries.length === 0) return Effect.succeed([]);
    return Effect.tryPromise({
        try: () =>
            db
                .insert(vocabularyEntry)
                .values(
                    entries.map(e => ({
                        id: e.id,
                        word: e.word.toLowerCase(),
                        category: e.category,
                        expects: e.expects ?? null,
                        codebaseId: e.codebaseId,
                        userId: e.userId
                    }))
                )
                .onConflictDoNothing()
                .returning(),
        catch: cause => new DatabaseError({ cause })
    });
}

export function updateVocabularyEntry(
    id: string,
    params: { word?: string; category?: string; expects?: string[] }
) {
    return Effect.tryPromise({
        try: async () => {
            const [updated] = await db
                .update(vocabularyEntry)
                .set({
                    ...(params.word !== undefined && { word: params.word.toLowerCase() }),
                    ...(params.category !== undefined && { category: params.category }),
                    ...(params.expects !== undefined && { expects: params.expects })
                })
                .where(eq(vocabularyEntry.id, id))
                .returning();
            return updated ?? null;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function deleteVocabularyEntry(id: string) {
    return Effect.tryPromise({
        try: async () => {
            const [deleted] = await db
                .delete(vocabularyEntry)
                .where(eq(vocabularyEntry.id, id))
                .returning();
            return deleted ?? null;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function getVocabularyEntry(id: string) {
    return Effect.tryPromise({
        try: async () => {
            const [found] = await db
                .select()
                .from(vocabularyEntry)
                .where(eq(vocabularyEntry.id, id));
            return found ?? null;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

const STANDARD_MODIFIERS = [
    "a", "an", "the", "is", "are", "was", "were",
    "with", "on", "to", "their", "not", "has", "have",
    "they", "it"
];

export function seedModifiers(codebaseId: string, userId: string) {
    return Effect.tryPromise({
        try: () =>
            db
                .insert(vocabularyEntry)
                .values(
                    STANDARD_MODIFIERS.map(word => ({
                        id: crypto.randomUUID(),
                        word,
                        category: "modifier",
                        expects: null,
                        codebaseId,
                        userId
                    }))
                )
                .onConflictDoNothing()
                .returning(),
        catch: cause => new DatabaseError({ cause })
    });
}

export function countVocabulary(codebaseId: string) {
    return Effect.tryPromise({
        try: async () => {
            const [result] = await db
                .select({ count: sql<number>`count(*)` })
                .from(vocabularyEntry)
                .where(eq(vocabularyEntry.codebaseId, codebaseId));
            return Number(result?.count ?? 0);
        },
        catch: cause => new DatabaseError({ cause })
    });
}
