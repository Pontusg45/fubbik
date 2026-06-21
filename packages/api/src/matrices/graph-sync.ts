// packages/api/src/matrices/graph-sync.ts
import { Effect } from "effect";

import { getCodeForCell, getMatrixView, getRulesForMatrix, isAgeAvailable, listMatrices } from "@fubbik/db/repository";
import { cypherVoid, escCypher } from "@fubbik/db/age/client";
import { deleteEdgesFrom, ensureVertex } from "@fubbik/db/age/sync";

import { logger } from "../logger";

/**
 * Projects behavior rules and the code they govern into the Apache AGE graph.
 *
 * Produces `behavior_rule` vertices (one per rule) and `governs` edges from each
 * rule to the `code_file` / `code_symbol` vertices it controls (via cell → cell-code
 * links). Mirrors `syncIndexToGraph` / `syncAnnotatesEdges` in code-index/service.ts.
 *
 * All optional steps are tolerant — a failure on one matrix/rule does not abort the sweep.
 */
export function syncBehaviorsToGraph(userId: string) {
    return Effect.gen(function* () {
        const available = yield* Effect.promise(() => isAgeAvailable());
        if (!available) return { synced: 0 };

        const matrices = yield* listMatrices(userId).pipe(Effect.catchAll(() => Effect.succeed([])));
        let synced = 0;

        for (const matrix of matrices) {
            const rules = yield* getRulesForMatrix(matrix.id).pipe(Effect.catchAll(() => Effect.succeed([])));
            if (rules.length === 0) continue;

            // Map ruleId -> cellIds via the full matrix view (cells carry ruleId).
            const view = yield* getMatrixView(matrix.id).pipe(
                Effect.catchAll(() => Effect.succeed({ dimensions: [], rules: [], cells: [] }))
            );
            const cellsByRule = new Map<string, string[]>();
            for (const cell of view.cells) {
                const list = cellsByRule.get(cell.ruleId) ?? [];
                list.push(cell.id);
                cellsByRule.set(cell.ruleId, list);
            }

            for (const rule of rules) {
                yield* ensureVertex("behavior_rule", rule.id);
                yield* cypherVoid(
                    `MATCH (r:behavior_rule {id: '${escCypher(rule.id)}'})
                     SET r.title = '${escCypher(rule.title)}', r.layer = '${escCypher(matrix.layer)}',
                         r.matrixId = '${escCypher(matrix.id)}', r.category = '${escCypher(rule.category ?? "")}'`
                ).pipe(Effect.catchAll(() => Effect.void));

                // Rebuild governs edges idempotently.
                yield* deleteEdgesFrom("governs", "behavior_rule", rule.id).pipe(Effect.catchAll(() => Effect.void));

                const cellIds = cellsByRule.get(rule.id) ?? [];
                for (const cellId of cellIds) {
                    const codeLinks = yield* getCodeForCell(cellId).pipe(Effect.catchAll(() => Effect.succeed([])));
                    for (const link of codeLinks) {
                        if (link.kind === "file") {
                            yield* cypherVoid(
                                `MATCH (r:behavior_rule {id: '${escCypher(rule.id)}'}), (f:code_file)
                                 WHERE f.id ENDS WITH '${escCypher(link.ref)}'
                                 MERGE (r)-[:governs {kind: 'file'}]->(f)`
                            ).pipe(Effect.catchAll(() => Effect.void));
                        } else if (link.kind === "symbol") {
                            yield* cypherVoid(
                                `MATCH (r:behavior_rule {id: '${escCypher(rule.id)}'}), (s:code_symbol)
                                 WHERE s.id ENDS WITH '${escCypher(link.ref)}'
                                 MERGE (r)-[:governs {kind: 'symbol'}]->(s)`
                            ).pipe(Effect.catchAll(() => Effect.void));
                        }
                    }
                }

                synced++;
            }
        }

        logger.info("Behavior rules synced to graph", { rules: synced });
        return { synced };
    });
}
