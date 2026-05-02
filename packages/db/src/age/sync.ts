// packages/db/src/age/sync.ts
import { cypherVoid, escCypher } from "./client";

export function ensureVertex(label: string, id: string) {
    return cypherVoid(`MERGE (:${label} {id: '${escCypher(id)}'})`);
}

export function deleteVertex(label: string, id: string) {
    return cypherVoid(`MATCH (v:${label} {id: '${escCypher(id)}'}) DETACH DELETE v`);
}

export function createEdge(
    edgeLabel: string,
    fromLabel: string,
    fromId: string,
    toLabel: string,
    toId: string,
    props: Record<string, string> = {}
) {
    const propsStr = Object.entries(props)
        .map(([k, v]) => `${k}: '${escCypher(v)}'`)
        .join(", ");
    const propsClause = propsStr ? ` {${propsStr}}` : "";
    return cypherVoid(
        `MATCH (a:${fromLabel} {id: '${escCypher(fromId)}'}), (b:${toLabel} {id: '${escCypher(toId)}'})
         CREATE (a)-[:${edgeLabel}${propsClause}]->(b)`
    );
}

export function deleteEdge(edgeLabel: string, props: Record<string, string>) {
    const conditions = Object.entries(props)
        .map(([k, v]) => `e.${k} = '${escCypher(v)}'`)
        .join(" AND ");
    return cypherVoid(
        `MATCH ()-[e:${edgeLabel}]-() WHERE ${conditions} DELETE e`
    );
}

export function deleteEdgesFrom(edgeLabel: string, fromLabel: string, fromId: string) {
    return cypherVoid(
        `MATCH (a:${fromLabel} {id: '${escCypher(fromId)}'})-[e:${edgeLabel}]->() DELETE e`
    );
}
