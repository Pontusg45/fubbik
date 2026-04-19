import { asc, eq, sql } from "drizzle-orm";
import { Effect } from "effect";

import { db, dbEffect } from "../index";
import { vocabularyEntry } from "../schema/vocabulary";

export function listVocabulary(codebaseId: string) {
    return dbEffect(() =>
            db
                .select()
                .from(vocabularyEntry)
                .where(eq(vocabularyEntry.codebaseId, codebaseId))
                .orderBy(asc(vocabularyEntry.category), asc(vocabularyEntry.word)));
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
    return dbEffect(async () => {
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
    return dbEffect(() =>
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
                .returning());
}

export function updateVocabularyEntry(
    id: string,
    params: { word?: string; category?: string; expects?: string[] }
) {
    return dbEffect(async () => {
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
        });
}

export function deleteVocabularyEntry(id: string) {
    return dbEffect(async () => {
            const [deleted] = await db
                .delete(vocabularyEntry)
                .where(eq(vocabularyEntry.id, id))
                .returning();
            return deleted ?? null;
        });
}

export function getVocabularyEntry(id: string) {
    return dbEffect(async () => {
            const [found] = await db
                .select()
                .from(vocabularyEntry)
                .where(eq(vocabularyEntry.id, id));
            return found ?? null;
        });
}

const STANDARD_MODIFIERS = [
    "a", "an", "the", "is", "are", "was", "were",
    "with", "on", "to", "their", "not", "has", "have",
    "they", "it"
];

export function seedModifiers(codebaseId: string, userId: string) {
    return dbEffect(() =>
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
                .returning());
}

export function countVocabulary(codebaseId: string) {
    return dbEffect(async () => {
            const [result] = await db
                .select({ count: sql<number>`count(*)` })
                .from(vocabularyEntry)
                .where(eq(vocabularyEntry.codebaseId, codebaseId));
            return Number(result?.count ?? 0);
        });
}
