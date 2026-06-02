import {
    getSpaceById,
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

function verifySpaceOwnership(spaceId: string, userId: string) {
    return getSpaceById(spaceId, userId).pipe(
        Effect.filterOrFail(
            (sp): sp is NonNullable<typeof sp> => sp !== null,
            () => new NotFoundError({ resource: "Space" })
        )
    );
}

export function listVocabulary(userId: string, spaceId: string) {
    return Effect.gen(function* () {
        yield* verifySpaceOwnership(spaceId, userId);
        return yield* listVocabularyRepo(spaceId);
    });
}

export function createEntry(
    userId: string,
    body: { word: string; category: string; expects?: string[]; spaceId: string }
) {
    return Effect.gen(function* () {
        yield* verifySpaceOwnership(body.spaceId, userId);

        // Auto-seed modifiers if this is the first entry for the space
        const count = yield* countVocabulary(body.spaceId);
        if (count === 0) {
            yield* seedModifiers(body.spaceId, userId);
        }

        const id = crypto.randomUUID();
        return yield* createVocabularyEntry({
            id,
            word: body.word,
            category: body.category,
            expects: body.expects,
            spaceId: body.spaceId,
            userId
        });
    });
}

export function createEntries(
    userId: string,
    body: {
        entries: Array<{ word: string; category: string; expects?: string[] }>;
        spaceId: string;
    }
) {
    return Effect.gen(function* () {
        yield* verifySpaceOwnership(body.spaceId, userId);
        return yield* createVocabularyEntries(
            body.entries.map(e => ({
                id: crypto.randomUUID(),
                word: e.word,
                category: e.category,
                expects: e.expects,
                spaceId: body.spaceId,
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
        yield* verifySpaceOwnership(entry.spaceId, userId);
        return yield* updateVocabularyEntry(id, body);
    });
}

export function deleteEntry(id: string, userId: string) {
    return Effect.gen(function* () {
        const entry = yield* getVocabularyEntry(id);
        if (!entry) return yield* Effect.fail(new NotFoundError({ resource: "Vocabulary entry" }));
        yield* verifySpaceOwnership(entry.spaceId, userId);
        return yield* deleteVocabularyEntry(id);
    });
}

export function parseStep(
    userId: string,
    body: { text: string; spaceId: string }
) {
    return Effect.gen(function* () {
        yield* verifySpaceOwnership(body.spaceId, userId);
        const vocab = yield* listVocabularyRepo(body.spaceId);
        const vocabEntries: VocabEntry[] = vocab.map(v => ({
            word: v.word,
            category: v.category,
            expects: v.expects
        }));
        return parseStepText(body.text, vocabEntries);
    });
}

export function suggestFromChunks(userId: string, spaceId: string) {
    return Effect.gen(function* () {
        yield* verifySpaceOwnership(spaceId, userId);
        const { chunks } = yield* listChunks({
            userId,
            codebaseId: spaceId,
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
