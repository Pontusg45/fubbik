import { Effect } from "effect";

import { cypher, escCypher } from "./client";

function parseAgtypeId(val: unknown): string {
    if (typeof val === "string") return val.replace(/^"|"$/g, "");
    return String(val);
}

export function findShortestPath(chunkIdA: string, chunkIdB: string) {
    // AGE 1.x does not support shortestPath() or list comprehensions.
    // We simulate a shortest-path check by attempting a variable-hop traversal
    // with a generous upper bound and returning the target id when reachable.
    // Callers receive null when no path exists; a truthy path array otherwise.
    return cypher(
        `MATCH (a:chunk {id: '${chunkIdA}'})-[*1..10]-(b:chunk {id: '${chunkIdB}'})
         RETURN b.id AS path LIMIT 1`,
        "path agtype"
    ).pipe(
        Effect.map(rows => {
            if (rows.length === 0) return null;
            const raw = (rows[0] as any)?.path;
            if (!raw) return null;
            // Return a two-element array [start, end] to indicate connectivity
            const target = typeof raw === "string" ? raw.replace(/^"|"$/g, "") : String(raw);
            return [chunkIdA, target] as string[];
        })
    );
}

export function getNeighborhood(chunkId: string, maxHops: number) {
    return cypher(
        `MATCH (c:chunk {id: '${chunkId}'})-[*1..${maxHops}]-(neighbor:chunk)
         RETURN DISTINCT neighbor.id AS id`,
        "id agtype"
    ).pipe(
        Effect.map(rows => rows.map((r: any) => parseAgtypeId(r.id)))
    );
}

export function getTransitiveDeps(requirementId: string) {
    return Effect.gen(function* () {
        const ancestorRows = yield* cypher(
            `MATCH (r:requirement {id: '${requirementId}'})-[:depends_on*]->(dep:requirement)
             RETURN dep.id AS id`,
            "id agtype"
        );
        const ancestors = ancestorRows.map((r: any) => parseAgtypeId(r.id));

        const descendantRows = yield* cypher(
            `MATCH (dep:requirement)-[:depends_on*]->(r:requirement {id: '${requirementId}'})
             RETURN dep.id AS id`,
            "id agtype"
        );
        const descendants = descendantRows.map((r: any) => parseAgtypeId(r.id));

        const allIds = [requirementId, ...ancestors, ...descendants];
        const idList = allIds.map(id => `'${id}'`).join(",");
        const edgeRows = yield* cypher(
            `MATCH (a:requirement)-[e:depends_on]->(b:requirement)
             WHERE a.id IN [${idList}] OR b.id IN [${idList}]
             RETURN a.id AS source, b.id AS target`,
            "source agtype, target agtype"
        );
        const edges = edgeRows.map((r: any) => ({
            source: parseAgtypeId(r.source),
            target: parseAgtypeId(r.target)
        }));

        return { ancestors, descendants, edges };
    });
}

export function checkCircular(requirementId: string, dependsOnId: string) {
    // "end" is a reserved keyword in AGE Cypher — use "dest" instead.
    return cypher(
        `MATCH (start:requirement {id: '${dependsOnId}'})-[:depends_on*]->(dest:requirement {id: '${requirementId}'})
         RETURN 1 AS found LIMIT 1`,
        "found agtype"
    ).pipe(
        Effect.map(rows => rows.length > 0)
    );
}

export function getChunksAffectedByRequirement(requirementId: string, hops: number) {
    return cypher(
        `MATCH (r:requirement {id: '${requirementId}'})-[:covers]->(c:chunk)-[:connects*0..${hops}]-(related:chunk)
         RETURN DISTINCT related.id AS id`,
        "id agtype"
    ).pipe(
        Effect.map(rows => rows.map((r: any) => parseAgtypeId(r.id)))
    );
}

export function getSubgraph(chunkIds: string[]) {
    const idList = chunkIds.map(id => `'${id}'`).join(",");
    return cypher(
        `MATCH (a:chunk)-[e:connects]->(b:chunk)
         WHERE a.id IN [${idList}] AND b.id IN [${idList}]
         RETURN a.id AS source, e.relation AS relation, b.id AS target`,
        "source agtype, relation agtype, target agtype"
    ).pipe(
        Effect.map(rows => rows.map((r: any) => ({
            source: parseAgtypeId(r.source),
            relation: parseAgtypeId(r.relation),
            target: parseAgtypeId(r.target)
        })))
    );
}

export function getHopDistances(referenceId: string, targetIds: string[]) {
    if (targetIds.length === 0) return Effect.succeed(new Map<string, number>());

    const idList = targetIds.map(id => `'${id}'`).join(",");
    return cypher(
        `MATCH (ref:chunk {id: '${referenceId}'}), (target:chunk)
         WHERE target.id IN [${idList}]
         MATCH p = shortestPath((ref)-[*]-(target))
         RETURN target.id AS id, length(p) AS hops`,
        "id agtype, hops agtype"
    ).pipe(
        Effect.map(rows => {
            const map = new Map<string, number>();
            for (const row of rows) {
                const id = parseAgtypeId((row as any).id);
                const hops = Number((row as any).hops);
                map.set(id, hops);
            }
            return map;
        })
    );
}

export function getConnectionDegrees(chunkIds: string[]) {
    if (chunkIds.length === 0) return Effect.succeed(new Map<string, number>());

    const idList = chunkIds.map(id => `'${escCypher(id)}'`).join(",");
    return cypher(
        `MATCH (c:chunk)-[e]-()
         WHERE c.id IN [${idList}]
         RETURN c.id AS id, count(e) AS degree`,
        "id agtype, degree agtype"
    ).pipe(
        Effect.map(rows => {
            const map = new Map<string, number>();
            for (const row of rows) {
                const id = parseAgtypeId((row as any).id);
                const degree = Number((row as any).degree);
                map.set(id, degree);
            }
            return map;
        })
    );
}

export function getGraphProximityBoost(
    anchorId: string,
    candidateIds: string[],
    maxHops: number
) {
    if (candidateIds.length === 0) return Effect.succeed(new Map<string, number>());

    const idList = candidateIds.map(id => `'${escCypher(id)}'`).join(",");
    return cypher(
        `MATCH (anchor:chunk {id: '${escCypher(anchorId)}'}), (target:chunk)
         WHERE target.id IN [${idList}]
         MATCH p = shortestPath((anchor)-[*1..${maxHops}]-(target))
         RETURN target.id AS id, length(p) AS hops`,
        "id agtype, hops agtype"
    ).pipe(
        Effect.map(rows => {
            const map = new Map<string, number>();
            for (const row of rows) {
                const id = parseAgtypeId((row as any).id);
                const hops = Number((row as any).hops);
                map.set(id, 1 / hops);
            }
            return map;
        }),
        Effect.catchAll(() => Effect.succeed(new Map<string, number>()))
    );
}

export function getDownstreamChunks(chunkId: string, maxHops: number) {
    return cypher(
        `MATCH (source:chunk {id: '${escCypher(chunkId)}'})-[:connects*1..${maxHops}]->(downstream:chunk)
         RETURN DISTINCT downstream.id AS id`,
        "id agtype"
    ).pipe(
        Effect.map(rows => rows.map((r: any) => parseAgtypeId(r.id))),
        Effect.catchAll(() => Effect.succeed([] as string[]))
    );
}

export interface Community {
    id: string;
    members: string[];
}

export function detectCommunities(chunkIds: string[], _maxHops: number) {
    if (chunkIds.length === 0) return Effect.succeed([] as Community[]);

    const idList = chunkIds.map(id => `'${escCypher(id)}'`).join(",");
    return cypher(
        `MATCH (a:chunk)-[e:connects]-(b:chunk)
         WHERE a.id IN [${idList}] AND b.id IN [${idList}]
         RETURN DISTINCT a.id AS source, b.id AS target`,
        "source agtype, target agtype"
    ).pipe(
        Effect.map(rows => {
            const adj = new Map<string, Set<string>>();
            for (const id of chunkIds) adj.set(id, new Set());
            for (const row of rows) {
                const source = parseAgtypeId((row as any).source);
                const target = parseAgtypeId((row as any).target);
                adj.get(source)?.add(target);
                adj.get(target)?.add(source);
            }

            const visited = new Set<string>();
            const communities: Community[] = [];

            for (const id of chunkIds) {
                if (visited.has(id)) continue;
                const members: string[] = [];
                const queue = [id];
                while (queue.length > 0) {
                    const current = queue.shift()!;
                    if (visited.has(current)) continue;
                    visited.add(current);
                    members.push(current);
                    for (const neighbor of adj.get(current) ?? []) {
                        if (!visited.has(neighbor)) queue.push(neighbor);
                    }
                }
                if (members.length > 1) {
                    communities.push({ id: members[0]!, members });
                }
            }

            return communities.sort((a, b) => b.members.length - a.members.length);
        })
    );
}

export function findBridgeChunks(chunkIds: string[]) {
    if (chunkIds.length === 0) return Effect.succeed([] as string[]);

    const idList = chunkIds.map(id => `'${escCypher(id)}'`).join(",");
    return cypher(
        `MATCH (a:chunk)-[e:connects]-(b:chunk)
         WHERE a.id IN [${idList}] AND b.id IN [${idList}]
         RETURN DISTINCT a.id AS source, b.id AS target`,
        "source agtype, target agtype"
    ).pipe(
        Effect.map(rows => {
            const adj = new Map<string, Set<string>>();
            for (const id of chunkIds) adj.set(id, new Set());
            for (const row of rows) {
                const source = parseAgtypeId((row as any).source);
                const target = parseAgtypeId((row as any).target);
                adj.get(source)?.add(target);
                adj.get(target)?.add(source);
            }

            const visited = new Set<string>();
            const disc = new Map<string, number>();
            const low = new Map<string, number>();
            const parent = new Map<string, string | null>();
            const bridges: string[] = [];
            let timer = 0;

            function dfs(u: string) {
                visited.add(u);
                disc.set(u, timer);
                low.set(u, timer);
                timer++;
                let children = 0;
                let isArticulation = false;

                for (const v of adj.get(u) ?? []) {
                    if (!visited.has(v)) {
                        children++;
                        parent.set(v, u);
                        dfs(v);
                        low.set(u, Math.min(low.get(u)!, low.get(v)!));
                        if (parent.get(u) === null && children > 1) isArticulation = true;
                        if (parent.get(u) !== null && low.get(v)! >= disc.get(u)!) isArticulation = true;
                    } else if (v !== parent.get(u)) {
                        low.set(u, Math.min(low.get(u)!, disc.get(v)!));
                    }
                }
                if (isArticulation) bridges.push(u);
            }

            for (const id of chunkIds) {
                if (!visited.has(id) && (adj.get(id)?.size ?? 0) > 0) {
                    parent.set(id, null);
                    dfs(id);
                }
            }

            return bridges;
        })
    );
}

export interface PathEdge {
    source: string;
    target: string;
    relation: string;
}

export interface DetailedPath {
    nodes: string[];
    edges: PathEdge[];
    hops: number;
}

export function findShortestPathWithDetails(chunkIdA: string, chunkIdB: string) {
    const escapedA = escCypher(chunkIdA);
    const escapedB = escCypher(chunkIdB);

    return cypher(
        `MATCH (a:chunk {id: '${escapedA}'})-[*1..10]-(b:chunk {id: '${escapedB}'})
         RETURN b.id AS found LIMIT 1`,
        "found agtype"
    ).pipe(
        Effect.flatMap(rows => {
            if (rows.length === 0) return Effect.succeed(null as DetailedPath | null);

            return cypher(
                `MATCH (x:chunk)-[e:connects]->(y:chunk)
                 RETURN x.id AS source, y.id AS target, e.relation AS relation`,
                "source agtype, target agtype, relation agtype"
            ).pipe(
                Effect.map(edgeRows => {
                    const adj = new Map<string, Array<{ neighbor: string; relation: string }>>();
                    const addEdge = (from: string, to: string, rel: string) => {
                        if (!adj.has(from)) adj.set(from, []);
                        adj.get(from)!.push({ neighbor: to, relation: rel });
                    };
                    for (const row of edgeRows) {
                        const s = parseAgtypeId((row as any).source);
                        const t = parseAgtypeId((row as any).target);
                        const r = parseAgtypeId((row as any).relation);
                        addEdge(s, t, r);
                        addEdge(t, s, r);
                    }

                    const visited = new Set<string>();
                    const parentMap = new Map<string, { from: string; relation: string } | null>();
                    const queue = [chunkIdA];
                    visited.add(chunkIdA);
                    parentMap.set(chunkIdA, null);

                    while (queue.length > 0) {
                        const current = queue.shift()!;
                        if (current === chunkIdB) break;
                        for (const { neighbor, relation } of adj.get(current) ?? []) {
                            if (!visited.has(neighbor)) {
                                visited.add(neighbor);
                                parentMap.set(neighbor, { from: current, relation });
                                queue.push(neighbor);
                            }
                        }
                    }

                    if (!parentMap.has(chunkIdB)) return null;

                    const nodes: string[] = [];
                    const edges: PathEdge[] = [];
                    let current: string | null = chunkIdB;
                    while (current !== null) {
                        nodes.unshift(current);
                        const p = parentMap.get(current);
                        if (p) {
                            edges.unshift({ source: p.from, target: current, relation: p.relation });
                            current = p.from;
                        } else {
                            current = null;
                        }
                    }

                    return { nodes, edges, hops: edges.length } as DetailedPath;
                })
            );
        }),
        Effect.catchAll(() => Effect.succeed(null as DetailedPath | null))
    );
}

export function getUpstreamChunks(chunkId: string, maxHops: number) {
    return cypher(
        `MATCH (upstream:chunk)-[:connects*1..${maxHops}]->(target:chunk {id: '${escCypher(chunkId)}'})
         RETURN DISTINCT upstream.id AS id`,
        "id agtype"
    ).pipe(
        Effect.map(rows => rows.map((r: any) => parseAgtypeId(r.id))),
        Effect.catchAll(() => Effect.succeed([] as string[]))
    );
}

export function getOrphanChunkIds() {
    // AGE 1.x does not support the anonymous pattern `NOT (c)-[]-()`.
    // Use the EXISTS subquery form instead.
    return cypher(
        `MATCH (c:chunk) WHERE NOT EXISTS { MATCH (c)-[]-() } RETURN c.id AS id`,
        "id agtype"
    ).pipe(
        Effect.map(rows => rows.map((r: any) => parseAgtypeId(r.id)))
    );
}
