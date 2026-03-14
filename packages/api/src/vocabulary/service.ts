import {
    getCodebaseById,
    listVocabulary as listVocabularyRepo,
    createVocabularyEntry,
    createVocabularyEntries,
    updateVocabularyEntry,
    deleteVocabularyEntry,
    getVocabularyEntry,
    seedModifiers,
    countVocabulary,
    listChunks
} from "@fubbik/db/repository";
import { Effect } from "effect";

import { NotFoundError } from "../errors";
import { parseStepText, type VocabEntry } from "./parser";
import { suggestVocabulary } from "./suggest";

function verifyCodebaseOwnership(codebaseId: string, userId: string) {
    return getCodebaseById(codebaseId, userId).pipe(
        Effect.filterOrFail(
            (cb): cb is NonNullable<typeof cb> => cb !== null,
            () => new NotFoundError({ resource: "Codebase" })
        )
    );
}

export function listVocabulary(userId: string, codebaseId: string) {
    return Effect.gen(function* () {
        yield* verifyCodebaseOwnership(codebaseId, userId);
        return yield* listVocabularyRepo(codebaseId);
    });
}

export function createEntry(
    userId: string,
    body: { word: string; category: string; expects?: string[]; codebaseId: string }
) {
    return Effect.gen(function* () {
        yield* verifyCodebaseOwnership(body.codebaseId, userId);

        // Auto-seed modifiers if this is the first entry for the codebase
        const count = yield* countVocabulary(body.codebaseId);
        if (count === 0) {
            yield* seedModifiers(body.codebaseId, userId);
        }

        const id = crypto.randomUUID();
        return yield* createVocabularyEntry({
            id,
            word: body.word,
            category: body.category,
            expects: body.expects,
            codebaseId: body.codebaseId,
            userId
        });
    });
}

export function createEntries(
    userId: string,
    body: {
        entries: Array<{ word: string; category: string; expects?: string[] }>;
        codebaseId: string;
    }
) {
    return Effect.gen(function* () {
        yield* verifyCodebaseOwnership(body.codebaseId, userId);
        return yield* createVocabularyEntries(
            body.entries.map(e => ({
                id: crypto.randomUUID(),
                word: e.word,
                category: e.category,
                expects: e.expects,
                codebaseId: body.codebaseId,
                userId
            }))
        );
    });
}

export function updateEntry(
    id: string,
    userId: string,
    body: { word?: string; category?: string; expects?: string[] }
) {
    return Effect.gen(function* () {
        const entry = yield* getVocabularyEntry(id);
        if (!entry) return yield* Effect.fail(new NotFoundError({ resource: "Vocabulary entry" }));
        yield* verifyCodebaseOwnership(entry.codebaseId, userId);
        return yield* updateVocabularyEntry(id, body);
    });
}

export function deleteEntry(id: string, userId: string) {
    return Effect.gen(function* () {
        const entry = yield* getVocabularyEntry(id);
        if (!entry) return yield* Effect.fail(new NotFoundError({ resource: "Vocabulary entry" }));
        yield* verifyCodebaseOwnership(entry.codebaseId, userId);
        return yield* deleteVocabularyEntry(id);
    });
}

export function parseStep(
    userId: string,
    body: { text: string; codebaseId: string }
) {
    return Effect.gen(function* () {
        yield* verifyCodebaseOwnership(body.codebaseId, userId);
        const vocab = yield* listVocabularyRepo(body.codebaseId);
        const vocabEntries: VocabEntry[] = vocab.map(v => ({
            word: v.word,
            category: v.category,
            expects: v.expects
        }));
        return parseStepText(body.text, vocabEntries);
    });
}

export function suggestFromChunks(userId: string, codebaseId: string) {
    return Effect.gen(function* () {
        yield* verifyCodebaseOwnership(codebaseId, userId);
        const { chunks } = yield* listChunks({
            userId,
            codebaseId,
            limit: 50,
            offset: 0
        });
        return yield* suggestVocabulary(
            chunks.map(c => ({
                title: c.title,
                content: c.content
            }))
        );
    });
}
