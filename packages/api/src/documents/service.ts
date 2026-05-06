import {
    createChunk as createChunkRepo,
    createDocument as createDocumentRepo,
    deleteDocument as deleteDocumentRepo,
    findOrCreateTag,
    getDocumentById,
    getDocumentBySourcePath,
    getDocumentChunks,
    getTagsForChunk,
    getTemplateById,
    listDocuments as listDocumentsRepo,
    listDocumentsWithTags as listDocumentsWithTagsRepo,
    searchDocumentChunks,
    setChunkCodebases,
    setChunkTags,
    updateChunk as updateChunkRepo,
    updateDocument as updateDocumentRepo
} from "@fubbik/db/repository";
import { Effect } from "effect";

import { parseDocFile } from "../chunks/parse-docs";
import { NotFoundError } from "../errors";
import { extractFields } from "../templates/field-extraction";
import type { FieldMapping } from "../templates/types";
import { renderMarkdown } from "./render-markdown";
import { splitMarkdown } from "./split-markdown";

function hashContent(content: string): string {
    const hasher = new Bun.CryptoHasher("sha256");
    hasher.update(new TextEncoder().encode(content));
    return hasher.digest("hex");
}

function resolveTagIds(tagNames: string[], userId: string) {
    return Effect.gen(function* () {
        const ids: string[] = [];
        for (const name of tagNames) {
            const tag = yield* findOrCreateTag(name, userId);
            if (tag) ids.push(tag.id);
        }
        return ids;
    });
}

export function importDocument(
    userId: string,
    sourcePath: string,
    rawContent: string,
    codebaseId?: string,
    templateId?: string
) {
    return Effect.gen(function* () {
        const contentHash = hashContent(rawContent);

        const existing = yield* getDocumentBySourcePath(sourcePath, codebaseId, userId);
        if (existing && existing.contentHash === contentHash) {
            const existingChunks = yield* getDocumentChunks(existing.id);
            const firstChunkId = existingChunks[0]?.id ?? null;
            return { document: existing, created: 0, updated: 0, status: "unchanged" as const, firstChunkId };
        }

        if (existing) {
            const syncResult = yield* syncDocument(existing.id, rawContent, userId, codebaseId);
            const syncChunks = yield* getDocumentChunks(existing.id);
            return { ...syncResult, firstChunkId: syncChunks[0]?.id ?? null };
        }

        // Template-aware import path
        if (templateId) {
            const template = yield* getTemplateById(templateId);

            if (template) {
                const parsed = parseDocFile(sourcePath, rawContent);
                const fieldMappings = (template.fieldMappings ?? []) as FieldMapping[];

                const { extracted, remainingContent } =
                    fieldMappings.length > 0
                        ? extractFields(rawContent, fieldMappings)
                        : { extracted: {}, remainingContent: rawContent };

                const mergedTags = [...new Set([...(template.tags ?? []), ...parsed.tags])];

                const docId = crypto.randomUUID();
                const doc = yield* createDocumentRepo({
                    id: docId,
                    title: parsed.title,
                    sourcePath,
                    contentHash,
                    description: undefined,
                    codebaseId,
                    userId
                });

                const tagIds = mergedTags.length > 0 ? yield* resolveTagIds(mergedTags, userId) : [];

                const chunkId = crypto.randomUUID();
                yield* createChunkRepo({
                    id: chunkId,
                    title: parsed.title,
                    content: remainingContent,
                    type: template.type,
                    userId,
                    rationale: extracted.rationale,
                    alternatives: extracted.alternatives,
                    consequences: extracted.consequences,
                    origin: "ai",
                    documentId: docId,
                    documentOrder: 0
                });

                if (extracted.summary) {
                    yield* updateChunkRepo(chunkId, { summary: extracted.summary });
                }

                if (tagIds.length > 0) {
                    yield* setChunkTags(chunkId, tagIds);
                }
                if (codebaseId) {
                    yield* setChunkCodebases(chunkId, [codebaseId]);
                }

                return { document: doc, created: 1, updated: 0, status: "created" as const, firstChunkId: chunkId };
            }
        }

        // Default (non-template) import path
        const split = splitMarkdown(rawContent, sourcePath);
        const docId = crypto.randomUUID();

        const doc = yield* createDocumentRepo({
            id: docId,
            title: split.title,
            sourcePath,
            contentHash,
            description: split.description,
            codebaseId,
            userId,
            splitLevel: split.splitLevel
        });

        const tagIds = split.tags.length > 0 ? yield* resolveTagIds(split.tags, userId) : [];

        let firstChunkId: string | null = null;
        for (const section of split.sections) {
            const chunkId = crypto.randomUUID();
            if (firstChunkId === null) firstChunkId = chunkId;
            yield* createChunkRepo({
                id: chunkId,
                title: section.title,
                content: section.content,
                type: "document",
                userId,
                documentId: docId,
                documentOrder: section.order
            });

            if (tagIds.length > 0) {
                yield* setChunkTags(chunkId, tagIds);
            }
            if (codebaseId) {
                yield* setChunkCodebases(chunkId, [codebaseId]);
            }
        }

        return { document: doc, created: split.sections.length, updated: 0, status: "created" as const, firstChunkId };
    });
}

export function syncDocument(
    documentId: string,
    rawContent: string,
    userId: string,
    codebaseId?: string
) {
    return Effect.gen(function* () {
        const doc = yield* getDocumentById(documentId);
        if (!doc || doc.userId !== userId) return yield* Effect.fail(new NotFoundError({ resource: "document" }));

        const contentHash = hashContent(rawContent);
        if (doc.contentHash === contentHash) {
            return { document: doc, created: 0, updated: 0, status: "unchanged" as const };
        }

        const split = splitMarkdown(rawContent, doc.sourcePath);
        const existingChunks = yield* getDocumentChunks(documentId);

        const normalize = (t: string) => t.trim().toLowerCase();
        const existingByTitle = new Map(existingChunks.map(c => [normalize(c.title), c]));
        const matchedIds = new Set<string>();

        let created = 0;
        let updated = 0;

        const tagIds = split.tags.length > 0 ? yield* resolveTagIds(split.tags, userId) : [];

        for (const section of split.sections) {
            const match = existingByTitle.get(normalize(section.title));

            if (match) {
                matchedIds.add(match.id);
                if (match.content !== section.content || match.documentOrder !== section.order) {
                    yield* updateChunkRepo(match.id, {
                        content: section.content,
                        documentOrder: section.order
                    });
                    updated++;
                }
            } else {
                const chunkId = crypto.randomUUID();
                yield* createChunkRepo({
                    id: chunkId,
                    title: section.title,
                    content: section.content,
                    type: "document",
                    userId,
                    documentId,
                    documentOrder: section.order
                });

                if (tagIds.length > 0) {
                    yield* setChunkTags(chunkId, tagIds);
                }
                if (codebaseId) {
                    yield* setChunkCodebases(chunkId, [codebaseId]);
                }
                created++;
            }
        }

        // Flag deleted sections as stale (don't delete)
        for (const existing of existingChunks) {
            if (!matchedIds.has(existing.id)) {
                yield* updateChunkRepo(existing.id, { documentOrder: undefined });
                const currentTags = yield* getTagsForChunk(existing.id);
                const tagNames = currentTags.map((t: { name: string }) => t.name);
                if (!tagNames.includes("stale")) {
                    const staleTagIds = yield* resolveTagIds([...tagNames, "stale"], userId);
                    yield* setChunkTags(existing.id, staleTagIds);
                }
            }
        }

        yield* updateDocumentRepo(documentId, {
            title: split.title,
            contentHash,
            description: split.description,
            splitLevel: split.splitLevel
        });

        return { document: doc, created, updated, status: "synced" as const };
    });
}

export function renderDocument(documentId: string, userId: string) {
    return Effect.gen(function* () {
        const doc = yield* getDocumentById(documentId);
        if (!doc || doc.userId !== userId) return yield* Effect.fail(new NotFoundError({ resource: "document" }));

        const chunks = yield* getDocumentChunks(documentId);
        if (chunks.length === 0) {
            return { document: doc, markdown: `# ${doc.title}\n` };
        }

        const tags = yield* getTagsForChunk(chunks[0]!.id);
        const tagNames = tags.map((t: { name: string }) => t.name);

        const firstChunk = chunks[0]!;
        const scope = firstChunk.scope && Object.keys(firstChunk.scope).length > 0
            ? firstChunk.scope
            : undefined;

        const sections = chunks.map((c, i) => ({
            title: c.title,
            content: c.content ?? "",
            order: c.documentOrder ?? i,
            rationale: c.rationale ?? undefined,
            alternatives: c.alternatives ?? undefined,
            consequences: c.consequences ?? undefined,
        }));

        const markdown = renderMarkdown({
            title: doc.title,
            type: firstChunk.type,
            tags: tagNames,
            scope,
            splitLevel: doc.splitLevel ?? 2,
            sections,
            sourcePath: doc.sourcePath,
        });

        return { document: doc, markdown };
    });
}

export function listDocuments(userId: string, codebaseId?: string) {
    return listDocumentsRepo(userId, codebaseId);
}

export function listDocumentsWithTags(userId: string, codebaseId?: string) {
    return listDocumentsWithTagsRepo(userId, codebaseId);
}

export function getDocument(documentId: string, userId: string) {
    return Effect.gen(function* () {
        const doc = yield* getDocumentById(documentId);
        if (!doc || doc.userId !== userId) return yield* Effect.fail(new NotFoundError({ resource: "document" }));
        const chunks = yield* getDocumentChunks(documentId);
        return { ...doc, chunks };
    });
}

export function searchDocuments(userId: string, query: string) {
    return searchDocumentChunks(userId, query);
}

export function removeDocument(documentId: string, userId: string) {
    return Effect.gen(function* () {
        const doc = yield* getDocumentById(documentId);
        if (!doc || doc.userId !== userId) return yield* Effect.fail(new NotFoundError({ resource: "document" }));
        return yield* deleteDocumentRepo(documentId);
    });
}
