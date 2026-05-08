import {
    createConnectionIfNotExists,
    listDocuments as listDocumentsRepo,
    listTemplates as listTemplatesRepo
} from "@fubbik/db/repository";
import { Effect } from "effect";

import { importDocument } from "../documents/service";
import { extractFields, parseHeadings } from "../templates/field-extraction";
import { matchTemplates } from "../templates/match-engine";
import type { ExtractedFields, FieldMapping, TemplateWithRules } from "../templates/types";
import { extractFrontmatter, parseDocFile } from "./parse-docs";

export const INDEX_FILE_NAMES = new Set(["index.md", "readme.md", "_index.md"]);

export function dirOf(filePath: string): string {
    const parts = filePath.split("/");
    parts.pop();
    return parts.join("/") || ".";
}

export function filenameOf(filePath: string): string {
    return (filePath.split("/").pop() ?? filePath).toLowerCase();
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

export function createFolderConnections(
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
                controller.enqueue(encode("file", { type: "file", path: file.path, status: "importing" }));
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
