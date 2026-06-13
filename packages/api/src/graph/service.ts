import { cypher } from "@fubbik/db/age/client";
import {
    detectCommunities,
    findBridgeChunks,
    getAllChunksMeta,
    getAllConnectionsForUser,
    getAllTagsWithTypes,
    getChunkSpaceMappings,
    getTagTypesForGraph
} from "@fubbik/db/repository";
import { Effect } from "effect";

export function getUserGraph(userId?: string, codebaseId?: string, workspaceId?: string) {
    return Effect.all(
        {
            chunks: getAllChunksMeta(userId, codebaseId, workspaceId),
            connections: getAllConnectionsForUser(userId),
            chunkTags: getAllTagsWithTypes(userId),
            tagTypes: getTagTypesForGraph(userId),
            chunkCodebases: workspaceId
                ? getChunkSpaceMappings(userId)
                : Effect.succeed([] as { chunkId: string; spaceId: string; spaceName: string }[])
        },
        { concurrency: "unbounded" }
    ).pipe(
        Effect.flatMap(result => {
            const chunkIds = result.chunks.map(c => c.id);
            return Effect.all({
                communities: detectCommunities(chunkIds, 1).pipe(Effect.catchAll(() => Effect.succeed([]))),
                bridges: findBridgeChunks(chunkIds).pipe(Effect.catchAll(() => Effect.succeed([] as string[])))
            }).pipe(
                Effect.flatMap(({ communities, bridges }) =>
                    Effect.tryPromise(async () => {
                        // Code file vertices
                        const codeFileRows = await Effect.runPromise(
                            cypher(
                                `MATCH (f:code_file) RETURN f.id AS id, f.language AS lang, f.lastIndexedAt AS indexed`,
                                "id agtype, lang agtype, indexed agtype"
                            ).pipe(Effect.catchAll(() => Effect.succeed([])))
                        );

                        // Code symbol vertices
                        const codeSymbolRows = await Effect.runPromise(
                            cypher(
                                `MATCH (s:code_symbol) RETURN s.id AS id, s.name AS name, s.kind AS kind, s.filePath AS fp, s.line AS line`,
                                "id agtype, name agtype, kind agtype, fp agtype, line agtype"
                            ).pipe(Effect.catchAll(() => Effect.succeed([])))
                        );

                        // Concept vertices with member chunks
                        const conceptRows = await Effect.runPromise(
                            cypher(
                                `MATCH (c:concept)-[r:embodies]->(chunk:chunk)
         RETURN c.id AS cid, c.label AS label, c.strength AS str, collect(chunk.id) AS members`,
                                "cid agtype, label agtype, str agtype, members agtype"
                            ).pipe(Effect.catchAll(() => Effect.succeed([])))
                        );

                        // Co-reference edges
                        const coRefRows = await Effect.runPromise(
                            cypher(
                                `MATCH (a:chunk)-[r:co_referenced]->(b:chunk) RETURN a.id AS src, b.id AS tgt, r.count AS cnt`,
                                "src agtype, tgt agtype, cnt agtype"
                            ).pipe(Effect.catchAll(() => Effect.succeed([])))
                        );

                        return {
                            ...result,
                            communities,
                            bridges,
                            codeFiles: codeFileRows.map(r => ({
                                id: String(r.id).replace(/"/g, ""),
                                language: String(r.lang).replace(/"/g, ""),
                                lastIndexedAt: String(r.indexed).replace(/"/g, "")
                            })),
                            codeSymbols: codeSymbolRows.map(r => ({
                                id: String(r.id).replace(/"/g, ""),
                                name: String(r.name).replace(/"/g, ""),
                                kind: String(r.kind).replace(/"/g, ""),
                                filePath: String(r.fp).replace(/"/g, ""),
                                line: Number(r.line)
                            })),
                            concepts: conceptRows.map(r => ({
                                id: String(r.cid).replace(/"/g, ""),
                                label: String(r.label).replace(/"/g, ""),
                                strength: Number(r.str),
                                memberChunkIds: String(r.members)
                                    .replace(/[\[\]"]/g, "")
                                    .split(",")
                                    .map(s => s.trim())
                                    .filter(Boolean)
                            })),
                            coRefEdges: coRefRows.map(r => ({
                                sourceId: String(r.src).replace(/"/g, ""),
                                targetId: String(r.tgt).replace(/"/g, ""),
                                count: Number(r.cnt)
                            }))
                        };
                    })
                )
            );
        })
    );
}
