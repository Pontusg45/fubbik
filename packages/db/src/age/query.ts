import { Effect } from "effect";

import { cypher } from "./client";

function parseAgtypeId(val: unknown): string {
    if (typeof val === "string") return val.replace(/^"|"$/g, "");
    return String(val);
}

export function findShortestPath(chunkIdA: string, chunkIdB: string) {
    return cypher(
        `MATCH p = shortestPath((a:chunk {id: '${chunkIdA}'})-[*]-(b:chunk {id: '${chunkIdB}'}))
         RETURN [n IN nodes(p) | n.id] AS path`,
        "path agtype"
    ).pipe(
        Effect.map(rows => {
            if (rows.length === 0) return null;
            const raw = (rows[0] as any)?.path;
            if (!raw) return null;
            return (typeof raw === "string" ? JSON.parse(raw) : raw) as string[];
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
    return cypher(
        `MATCH (start:requirement {id: '${dependsOnId}'})-[:depends_on*]->(end:requirement {id: '${requirementId}'})
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

export function getOrphanChunkIds() {
    return cypher(
        `MATCH (c:chunk) WHERE NOT (c)-[]-() RETURN c.id AS id`,
        "id agtype"
    ).pipe(
        Effect.map(rows => rows.map((r: any) => parseAgtypeId(r.id)))
    );
}
