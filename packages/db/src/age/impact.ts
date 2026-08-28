// packages/db/src/age/impact.ts
import { Effect } from "effect";

import { DatabaseError } from "../errors";
import { cypher, escCypher } from "./client";

const DISTANCE_DECAY: Record<number, number> = { 1: 0.9, 2: 0.5, 3: 0.2 };

const RELATION_WEIGHT: Record<string, number> = {
    depends_on: 1.0,
    extends: 0.8,
    part_of: 0.7,
    references: 0.3,
    related_to: 0.2
};

export interface ImpactTarget {
    chunkId: string;
    degree: number;
    hops: number;
    path: string[];
}

export function computeImpactRipple(changedChunkId: string): Effect.Effect<ImpactTarget[], DatabaseError> {
    return Effect.gen(function* () {
        const rows = yield* cypher(
            `MATCH (source:chunk {id: '${escCypher(changedChunkId)}'})-[r:connects*1..3]->(downstream:chunk)
             WHERE downstream.id <> '${escCypher(changedChunkId)}'
             RETURN downstream.id AS did, length(r) AS hops, [rel IN r | rel.relation] AS path`,
            "did agtype, hops agtype, path agtype"
        );

        const bestByChunk = new Map<string, ImpactTarget>();

        for (const row of rows) {
            const chunkId = String(row.did).replace(/"/g, "");
            const hops = Number(row.hops);
            const pathRaw = String(row.path);
            const relations = pathRaw
                .replace(/[[\]"]/g, "")
                .split(",")
                .map(s => s.trim())
                .filter(Boolean);

            const distanceFactor = DISTANCE_DECAY[hops] ?? 0.1;
            const relationFactor = relations.reduce((min, rel) => Math.min(min, RELATION_WEIGHT[rel] ?? 0.2), 1.0);
            const degree = distanceFactor * relationFactor;

            if (degree <= 0.1) continue;

            const existing = bestByChunk.get(chunkId);
            if (!existing || degree > existing.degree) {
                bestByChunk.set(chunkId, { chunkId, degree, hops, path: relations });
            }
        }

        return Array.from(bestByChunk.values());
    });
}
