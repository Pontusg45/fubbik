import {
    archiveChunk as archiveChunkRepo,
    batchFetchDeltas,
    createChunk as createChunkRepo,
    createConnectionIfNotExists,
    createVersion,
    deleteChunk as deleteChunkRepo,
    deleteMany as deleteManyRepo,
    exportAllChunks as exportAllChunksRepo,
    findNeighborsByChunkId,
    findOrCreateTag,
    getAppliesToForChunk,
    getChunkById,
    getChunkConnections,
    getCodebasesForChunk,
    getCodebasesForChunks,
    getFileRefsForChunk,
    getDeltasForChunk as getDeltasForChunkRepo,
    getDocumentById,
    getNextVersionNumber,
    getRequirementsForChunks,
    getTagsForChunk,
    getVersionsByChunkId,
    getVersionsByTag,
    getDistinctUpdateTags,
    listArchivedChunks as listArchivedChunksRepo,
    listChunks as listChunksRepo,
    listDocuments as listDocumentsRepo,
    listTemplates as listTemplatesRepo,
    mergeChunks as mergeChunksRepo,
    restoreChunk as restoreChunkRepo,
    semanticSearch as semanticSearchRepo,
    setChunkCodebases,
    setChunkTags,
    updateChunk as updateChunkRepo
} from "@fubbik/db/repository";
import { Effect } from "effect";

import type { DatabaseError } from "@fubbik/db/errors";
import { importDocument } from "../documents/service";
import { enrichChunk } from "../enrich/service";
import { resolveChunks } from "../features/resolve";
import { NotFoundError, ValidationError } from "../errors";
import { events, EVENTS } from "../events/bus";
import { generateQueryEmbedding } from "../ollama/client";
import { extractFields, parseHeadings } from "../templates/field-extraction";
import { matchTemplates } from "../templates/match-engine";
import type { ExtractedFields, FieldMapping, TemplateWithRules } from "../templates/types";
import { logger } from "../logger";
import { computeHealthScore } from "./health-score";
import { extractFrontmatter, parseDocFile } from "./parse-docs";

export function listChunks(
    userId: string | undefined,
    query: {
        type?: string;
        search?: string;
        limit?: string;
        offset?: string;
        exclude?: string;
        scope?: string;
        alias?: string;
        sort?: "newest" | "oldest" | "alpha" | "updated";
        tags?: string;
        tagMode?: "any" | "all";
        after?: string;
        enrichment?: "missing" | "complete";
        minConnections?: string;
        codebaseId?: string;
        workspaceId?: string;
        global?: string;
        origin?: string;
        reviewStatus?: string;
        allCodebases?: string;
    },
    activeFeatureIds: string[] = []
) {
    const limit = Math.min(Number(query.limit ?? 50), 100);
    const offset = Number(query.offset ?? 0);
    const exclude = query.exclude ? query.exclude.split(",").map(s => s.trim()) : undefined;
    const scope = query.scope
        ? Object.fromEntries(
              query.scope
                  .split(",")
                  .map(s => s.trim().split(":"))
                  .filter(p => p.length === 2) as [string, string][]
          )
        : undefined;
    const parsedTags = query.tags
        ?.split(",")
        .map(s => s.trim())
        .filter(Boolean);
    const tags = parsedTags?.length ? parsedTags : undefined;
    const after = query.after ? new Date(Date.now() - Number(query.after) * 86400000) : undefined;
    const minConnections = query.minConnections ? Number(query.minConnections) : undefined;
    const globalOnly = query.global === "true";
    const searchAllCodebases = query.allCodebases === "true";
    return listChunksRepo({
        userId,
        type: query.type,
        search: query.search,
        exclude,
        scope,
        alias: query.alias,
        sort: query.sort,
        tags,
        tagMode: query.tagMode,
        after,
        enrichment: query.enrichment,
        minConnections,
        codebaseId: searchAllCodebases ? undefined : query.codebaseId,
        workspaceId: searchAllCodebases ? undefined : query.workspaceId,
        globalOnly: searchAllCodebases ? false : globalOnly,
        origin: query.origin,
        reviewStatus: query.reviewStatus,
        limit,
        offset
    }).pipe(
        Effect.flatMap(result => {
            if (!searchAllCodebases || result.chunks.length === 0) {
                return Effect.succeed({ ...result, limit, offset });
            }
            return getCodebasesForChunks(result.chunks.map(c => c.id)).pipe(
                Effect.map(codebaseMap => {
                    const lookup = new Map<string, string[]>();
                    for (const entry of codebaseMap) {
                        const existing = lookup.get(entry.chunkId) ?? [];
                        existing.push(entry.codebaseName);
                        lookup.set(entry.chunkId, existing);
                    }
                    const chunks = result.chunks.map(c => ({
                        ...c,
                        codebaseNames: lookup.get(c.id) ?? []
                    }));
                    return { ...result, chunks, limit, offset };
                })
            );
        })
    ).pipe(
        Effect.flatMap(result => {
            if (activeFeatureIds.length === 0 || result.chunks.length === 0) {
                return Effect.succeed(result);
            }
            const chunkIds = result.chunks.map((c: { id: string }) => c.id);
            return batchFetchDeltas(chunkIds, activeFeatureIds).pipe(
                Effect.map(deltas => ({
                    ...result,
                    chunks: resolveChunks(result.chunks, activeFeatureIds, deltas)
                }))
            );
        })
    );
}

export function getChunkDetail(chunkId: string, userId?: string, activeFeatureIds: string[] = []) {
    return getChunkById(chunkId, userId).pipe(
        Effect.flatMap(found => (found ? Effect.succeed(found) : Effect.fail(new NotFoundError({ resource: "Chunk" })))),
        Effect.flatMap(found =>
            Effect.all({
                chunk: Effect.succeed(found),
                connections: getChunkConnections(chunkId),
                codebases: getCodebasesForChunk(chunkId),
                appliesTo: getAppliesToForChunk(chunkId),
                fileReferences: getFileRefsForChunk(chunkId),
                tags: getTagsForChunk(chunkId),
                requirements: getRequirementsForChunks([chunkId]),
                allDeltas: getDeltasForChunkRepo(chunkId)
            })
        ),
        Effect.map(result => {
            const chunkRequirements = result.requirements.filter(r => r.chunkId === chunkId);
            const requirementCount = chunkRequirements.length;
            const allRequirementsPassing = requirementCount > 0 && chunkRequirements.every(r => r.status === "passing");
            const healthScore = computeHealthScore({
                content: result.chunk.content,
                updatedAt: result.chunk.updatedAt,
                summary: result.chunk.summary,
                rationale: result.chunk.rationale,
                alternatives: result.chunk.alternatives,
                consequences: result.chunk.consequences,
                connectionCount: result.connections.length,
                hasEmbedding: result.chunk.embedding != null,
                requirementCount,
                allRequirementsPassing,
                referencedInSession: false
            });

            // Resolve chunk through active features
            let resolvedChunk = result.chunk as Record<string, unknown>;
            const _appliedFeatures: string[] = [];
            const _hasDeltas = result.allDeltas.length > 0;
            if (activeFeatureIds.length > 0 && result.allDeltas.length > 0) {
                const activeDeltas = result.allDeltas
                    .filter(d => activeFeatureIds.includes(d.featureId))
                    .sort((a, b) => a.featurePriority - b.featurePriority);
                for (const d of activeDeltas) {
                    resolvedChunk = { ...resolvedChunk, ...(d.delta as Record<string, unknown>) };
                    _appliedFeatures.push(d.featureId);
                }
            }

            return {
                ...result,
                chunk: resolvedChunk,
                healthScore,
                _appliedFeatures,
                _hasDeltas,
                deltas: result.allDeltas
            };
        })
    );
}

type ResolvedDocumentLinkage = {
    documentId: string | undefined;
    documentOrder: number | undefined;
};

function resolveDocumentLinkageForNewChunk(
    userId: string,
    rawDocumentId?: string,
    rawOrder?: number
): Effect.Effect<ResolvedDocumentLinkage, DatabaseError | ValidationError> {
    const trimmed =
        typeof rawDocumentId === "string" && rawDocumentId.trim().length > 0 ? rawDocumentId.trim() : undefined;

    if (!trimmed) {
        return Effect.succeed<ResolvedDocumentLinkage>({
            documentId: undefined,
            documentOrder: undefined
        });
    }

    return getDocumentById(trimmed).pipe(
        Effect.flatMap(doc => {
            if (!doc || doc.userId !== userId) {
                return Effect.fail(new ValidationError({ message: "Invalid or unknown document for documentId" }));
            }
            return Effect.succeed<ResolvedDocumentLinkage>({
                documentId: trimmed,
                documentOrder: rawOrder
            });
        })
    );
}

export function createChunk(
    userId: string,
    body: {
        title: string;
        content?: string;
        type?: string;
        tags?: string[];
        codebaseIds?: string[];
        rationale?: string;
        alternatives?: string[];
        consequences?: string;
        origin?: string;
        documentId?: string;
        documentOrder?: number;
        updateTag?: string;
    }
) {
    const id = crypto.randomUUID();
    const origin = body.origin ?? "human";
    return resolveDocumentLinkageForNewChunk(userId, body.documentId, body.documentOrder).pipe(
        Effect.flatMap(({ documentId, documentOrder }) =>
            createChunkRepo({
                id,
                title: body.title,
                content: body.content ?? "",
                type: body.type ?? "note",
                userId,
                rationale: body.rationale,
                alternatives: body.alternatives,
                consequences: body.consequences,
                origin,
                reviewStatus: origin === "ai" ? "draft" : "approved",
                documentId,
                documentOrder
            })
        ),
        Effect.tap(() => {
            if (body.tags && body.tags.length > 0) {
                return Effect.all(body.tags.map(name => findOrCreateTag(name, userId)), { concurrency: 5 }).pipe(
                    Effect.flatMap(tags => setChunkTags(id, tags.map(t => t.id)))
                );
            }
            return Effect.void;
        }),
        Effect.tap(() => {
            if (body.codebaseIds && body.codebaseIds.length > 0) {
                return setChunkCodebases(id, body.codebaseIds);
            }
            return Effect.void;
        }),
        Effect.tap(() => {
            events.emit(EVENTS.CHUNK_CREATED, { chunkId: id, userId });
            return Effect.void;
        }),
        Effect.tap(() => {
            if (body.updateTag) {
                return createVersion({
                    id: crypto.randomUUID(),
                    chunkId: id,
                    version: 0,
                    title: "",
                    content: "",
                    type: "",
                    tags: [],
                    updateTag: body.updateTag
                });
            }
            return Effect.void;
        })
    );
}

export function updateChunk(
    chunkId: string,
    userId: string,
    body: {
        title?: string;
        content?: string;
        type?: string;
        tags?: string[];
        codebaseIds?: string[];
        summary?: string | null;
        aliases?: string[];
        notAbout?: string[];
        scope?: Record<string, string>;
        rationale?: string;
        alternatives?: string[];
        consequences?: string;
        origin?: string;
        reviewStatus?: string;
        isEntryPoint?: boolean;
        updateTag?: string;
    }
) {
    return getChunkById(chunkId, userId).pipe(
        Effect.flatMap(existing => (existing ? Effect.succeed(existing) : Effect.fail(new NotFoundError({ resource: "Chunk" })))),
        Effect.flatMap(existing => Effect.all({ existing: Effect.succeed(existing), version: getNextVersionNumber(chunkId) })),
        Effect.flatMap(({ existing, version }) =>
            createVersion({
                id: crypto.randomUUID(),
                chunkId,
                version,
                title: existing.title,
                content: existing.content,
                type: existing.type,
                tags: [],
                rationale: existing.rationale,
                alternatives: existing.alternatives,
                consequences: existing.consequences,
                scope: existing.scope,
                updateTag: body.updateTag
            })
        ),
        Effect.flatMap(() => {
            const { tags: _tags, codebaseIds: _codebaseIds, updateTag: _updateTag, ...repoBody } = body;
            const updateData: Record<string, unknown> = { ...repoBody };
            if (body.reviewStatus !== undefined) {
                updateData.reviewedBy = userId;
                updateData.reviewedAt = new Date();
            }
            if (Object.keys(updateData).length === 0) return Effect.void;
            return updateChunkRepo(chunkId, updateData as Parameters<typeof updateChunkRepo>[1]).pipe(Effect.asVoid);
        }),
        Effect.tap(() => {
            if (body.tags) {
                return Effect.all(body.tags.map(name => findOrCreateTag(name, userId)), { concurrency: 5 }).pipe(
                    Effect.flatMap(tags => setChunkTags(chunkId, tags.map(t => t.id)))
                );
            }
            return Effect.void;
        }),
        Effect.tap(() => {
            if (body.codebaseIds) {
                return setChunkCodebases(chunkId, body.codebaseIds);
            }
            return Effect.void;
        }),
        Effect.tap(() => {
            if (body.title !== undefined || body.content !== undefined) {
                Effect.runPromise(enrichChunk(chunkId)).catch(err => {
                    logger.error(`[enrich] Failed to re-enrich chunk ${chunkId}:`, { err });
                });
            }
            events.emit(EVENTS.CHUNK_UPDATED, { chunkId, userId });
            return Effect.void;
        })
    );
}

export function getChunkHistory(chunkId: string, userId?: string) {
    return getChunkById(chunkId, userId).pipe(
        Effect.flatMap(existing => (existing ? Effect.succeed(existing) : Effect.fail(new NotFoundError({ resource: "Chunk" })))),
        Effect.flatMap(() => getVersionsByChunkId(chunkId))
    );
}

interface NeighborItem {
    id: string;
    title: string;
    summary: string | null;
    type: string;
    distance: number;
}

export interface NeighborsResult {
    neighbors: NeighborItem[];
    note: string | null;
}

export function getChunkNeighbors(chunkId: string, userId: string, k: number) {
    return getChunkById(chunkId, userId).pipe(
        Effect.flatMap(existing => (existing ? Effect.succeed(existing) : Effect.fail(new NotFoundError({ resource: "Chunk" })))),
        Effect.flatMap(source => {
            if (!source.embedding) {
                return Effect.succeed<NeighborsResult>({
                    neighbors: [],
                    note: "Chunk has no embedding — run enrichment first."
                });
            }
            return findNeighborsByChunkId(chunkId, userId, k).pipe(
                Effect.map((neighbors): NeighborsResult => ({ neighbors, note: null }))
            );
        })
    );
}

export function exportChunks(userId?: string) {
    return exportAllChunksRepo(userId);
}

export function importChunks(userId: string, chunks: { title: string; content?: string; type?: string; tags?: string[] }[]) {
    return Effect.all(
        chunks.map(c => createChunk(userId, c)),
        { concurrency: 10 }
    );
}

const INDEX_FILE_NAMES = new Set(["index.md", "readme.md", "_index.md"]);

function dirOf(filePath: string): string {
    const parts = filePath.split("/");
    parts.pop();
    return parts.join("/") || ".";
}

function filenameOf(filePath: string): string {
    return (filePath.split("/").pop() ?? filePath).toLowerCase();
}

export function importDocs(
    userId: string,
    files: { path: string; content: string }[],
    codebaseId: string,
    templateOverrides?: Record<string, string | null>
) {
    const results: { created: number; skipped: number; connections: number; errors: { path: string; error: string }[] } = {
        created: 0,
        skipped: 0,
        connections: 0,
        errors: []
    };

    const fileChunks = new Map<string, string>();

    return Effect.forEach(
        files,
        file => {
            const templateId = templateOverrides?.[file.path] ?? undefined;
            return importDocument(userId, file.path, file.content, codebaseId, templateId ?? undefined).pipe(
                Effect.map(result => {
                    if (result.status === "unchanged") {
                        results.skipped++;
                    } else {
                        results.created += result.created;
                    }
                    if (result.firstChunkId) {
                        fileChunks.set(file.path, result.firstChunkId);
                    }
                }),
                Effect.catchAll(err => {
                    results.errors.push({ path: file.path, error: String(err) });
                    return Effect.void;
                })
            );
        },
        { concurrency: 5 }
    ).pipe(
        Effect.flatMap(() => createFolderConnections(userId, fileChunks, codebaseId)),
        Effect.map(connectionsCreated => {
            results.connections = connectionsCreated;
            return results;
        })
    );
}

export function importDocsStream(
    userId: string,
    files: { path: string; content: string }[],
    codebaseId: string,
    templateOverrides?: Record<string, string | null>
): ReadableStream {
    const encoder = new TextEncoder();

    function encode(eventType: string, data: Record<string, unknown>) {
        return encoder.encode(`event: ${eventType}\ndata: ${JSON.stringify(data)}\n\n`);
    }

    return new ReadableStream({
        async start(controller) {
            const fileChunks = new Map<string, string>();
            let created = 0;
            let skipped = 0;
            let errors = 0;
            const startTime = Date.now();

            for (const file of files) {
                try {
                    const templateId = templateOverrides?.[file.path] ?? undefined;
                    const result = await Effect.runPromise(
                        importDocument(userId, file.path, file.content, codebaseId, templateId ?? undefined)
                    );
                    if (result.status === "unchanged") {
                        skipped++;
                        controller.enqueue(encode("file", { type: "file", path: file.path, status: "unchanged" }));
                    } else {
                        created += result.created;
                        if (result.firstChunkId) {
                            fileChunks.set(file.path, result.firstChunkId);
                        }
                        controller.enqueue(encode("file", { type: "file", path: file.path, status: "created", created: result.created }));
                    }
                } catch (err) {
                    errors++;
                    controller.enqueue(encode("file", { type: "file", path: file.path, status: "error", error: String(err) }));
                }
            }

            let connections = 0;
            try {
                connections = await Effect.runPromise(createFolderConnections(userId, fileChunks, codebaseId));
            } catch {}

            controller.enqueue(encode("done", { type: "done", created, skipped, errors, connections, elapsed: Date.now() - startTime }));
            controller.close();
        },
    });
}

function createFolderConnections(
    _userId: string,
    fileChunks: Map<string, string>,
    _codebaseId: string
) {
    return Effect.gen(function* () {
        const byDir = new Map<string, { path: string; chunkId: string; isIndex: boolean }[]>();
        for (const [path, chunkId] of fileChunks) {
            const dir = dirOf(path);
            if (!byDir.has(dir)) byDir.set(dir, []);
            byDir.get(dir)!.push({ path, chunkId, isIndex: INDEX_FILE_NAMES.has(filenameOf(path)) });
        }

        let connectionsCreated = 0;

        for (const [, entries] of byDir) {
            const indexEntry = entries.find(e => e.isIndex);
            if (!indexEntry) continue;

            const nonIndexEntries = entries.filter(e => !e.isIndex);
            for (const entry of nonIndexEntries) {
                const created = yield* createConnectionIfNotExists({
                    id: crypto.randomUUID(),
                    sourceId: entry.chunkId,
                    targetId: indexEntry.chunkId,
                    relation: "part_of",
                    origin: "import",
                    reviewStatus: "approved"
                });
                if (created) connectionsCreated++;
            }
        }

        return connectionsCreated;
    });
}

export function deleteChunk(chunkId: string, userId: string) {
    return deleteChunkRepo(chunkId, userId).pipe(
        Effect.flatMap(deleted => (deleted ? Effect.succeed(deleted) : Effect.fail(new NotFoundError({ resource: "Chunk" }))))
    );
}

export function deleteMany(ids: string[], userId: string) {
    return deleteManyRepo(ids, userId);
}

export function archiveChunk(chunkId: string, userId: string) {
    return archiveChunkRepo(chunkId, userId).pipe(
        Effect.flatMap(archived => (archived ? Effect.succeed(archived) : Effect.fail(new NotFoundError({ resource: "Chunk" }))))
    );
}

export function restoreChunk(chunkId: string, userId: string) {
    return restoreChunkRepo(chunkId, userId).pipe(
        Effect.flatMap(restored => (restored ? Effect.succeed(restored) : Effect.fail(new NotFoundError({ resource: "Chunk" }))))
    );
}

export function listArchivedChunks(userId: string, codebaseId?: string) {
    return listArchivedChunksRepo(userId, codebaseId);
}

export function semanticSearch(userId: string | undefined, query: { q: string; limit?: string; exclude?: string; scope?: string }) {
    const limit = Math.min(Number(query.limit ?? 5), 20);
    const exclude = query.exclude ? query.exclude.split(",").map(s => s.trim()) : undefined;
    const scope = query.scope
        ? Object.fromEntries(
              query.scope
                  .split(",")
                  .map(s => s.trim().split(":"))
                  .filter(p => p.length === 2) as [string, string][]
          )
        : undefined;

    return generateQueryEmbedding(query.q).pipe(
        Effect.flatMap(embedding => semanticSearchRepo({ embedding, userId, exclude, scope, limit }))
    );
}

export interface PreviewFileResult {
    path: string;
    title: string;
    suggestedTemplate: {
        id: string;
        name: string;
        score: number;
        type: string;
        tags: string[];
        extractedFields: {
            rationale?: string;
            alternatives?: string[];
            consequences?: string;
            summary?: string;
            scope?: Record<string, string>;
        };
    } | null;
    parsed: {
        title: string;
        type: string;
        tags: string[];
        content: string;
    };
}

export function getExistingHashes(codebaseId: string, userId: string) {
    return Effect.gen(function* () {
        const docs = yield* listDocumentsRepo(userId, codebaseId);
        const hashes: Record<string, string> = {};
        for (const doc of docs) {
            if (doc.sourcePath && doc.contentHash) {
                hashes[doc.sourcePath] = doc.contentHash;
            }
        }
        return hashes;
    });
}

export function previewImportDocs(
    userId: string,
    files: { path: string; content: string }[],
    codebaseId: string
) {
    return Effect.gen(function* () {
        const existingHashes = yield* getExistingHashes(codebaseId, userId);
        const allTemplates = yield* listTemplatesRepo(userId);

        const templatesWithRules: TemplateWithRules[] = allTemplates
            .filter(t => t.matchRules != null)
            .map(t => ({
                id: t.id,
                name: t.name,
                type: t.type,
                matchRules: t.matchRules as TemplateWithRules["matchRules"],
                fieldMappings: (t.fieldMappings ?? null) as FieldMapping[] | null,
                priority: t.priority ?? 0,
                tags: (t.tags ?? null) as string[] | null,
            }));

        const results: PreviewFileResult[] = [];

        for (const file of files) {
            const parsed = parseDocFile(file.path, file.content);
            const { frontmatter } = extractFrontmatter(file.content);
            const headings = parseHeadings(file.content);

            const match = matchTemplates({ headings, frontmatter }, templatesWithRules);

            let suggestedTemplate: PreviewFileResult["suggestedTemplate"] = null;

            if (match !== null) {
                const matchedTemplate = templatesWithRules.find(t => t.id === match.templateId);
                const fieldMappings = matchedTemplate?.fieldMappings ?? [];
                const { extracted } = fieldMappings.length > 0
                    ? extractFields(file.content, fieldMappings)
                    : { extracted: {} as ExtractedFields };

                const mergedTags = [...new Set([...(match.tags ?? []), ...parsed.tags])];

                suggestedTemplate = {
                    id: match.templateId,
                    name: match.templateName,
                    score: match.score,
                    type: match.type,
                    tags: mergedTags,
                    extractedFields: {
                        ...(extracted.rationale !== undefined && { rationale: extracted.rationale }),
                        ...(extracted.alternatives !== undefined && { alternatives: extracted.alternatives }),
                        ...(extracted.consequences !== undefined && { consequences: extracted.consequences }),
                        ...(extracted.summary !== undefined && { summary: extracted.summary }),
                        ...(extracted.scope !== undefined && { scope: extracted.scope }),
                    },
                };
            }

            results.push({
                path: file.path,
                title: parsed.title,
                suggestedTemplate,
                parsed: {
                    title: parsed.title,
                    type: parsed.type,
                    tags: parsed.tags,
                    content: parsed.content,
                },
            });
        }

        return { files: results, existingHashes };
    });
}

/**
 * Merge two chunks belonging to the same user. See repo `mergeChunks` for
 * the reparenting semantics. Refuses same-id merges up front so the UI can
 * display a clean 400 rather than a cryptic DB error.
 */
export function mergeChunks(userId: string, sourceId: string, targetId: string) {
    return Effect.gen(function* () {
        if (sourceId === targetId) {
            return yield* Effect.fail(new ValidationError({ message: "Cannot merge a chunk into itself" }));
        }
        return yield* mergeChunksRepo(sourceId, targetId, userId);
    });
}

export function listUpdatesByTag(userId: string, tag: string, codebaseId?: string) {
    return getVersionsByTag(tag, userId, codebaseId).pipe(
        Effect.map(versions => versions.map(v => ({
            versionId: v.versionId,
            chunkId: v.chunkId,
            chunkTitle: v.chunkTitle,
            updateTag: v.updateTag,
            version: v.version,
            createdAt: v.createdAt,
            before: v.version === 0
                ? { title: null, content: null, type: null, rationale: null, alternatives: null, consequences: null, scope: null }
                : { title: v.title, content: v.content, type: v.type, rationale: v.rationale, alternatives: v.alternatives, consequences: v.consequences, scope: v.scope },
            after: {
                title: v.chunkTitle,
                content: v.chunkContent,
                type: v.chunkType,
                rationale: v.chunkRationale,
                alternatives: v.chunkAlternatives,
                consequences: v.chunkConsequences,
                scope: v.chunkScope,
            }
        })))
    );
}

export function listUpdateTags(userId: string, codebaseId?: string) {
    return getDistinctUpdateTags(userId, codebaseId);
}
