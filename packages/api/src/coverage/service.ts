import { getChunkCoverage, getChunkCoverageMatrix } from "@fubbik/db/repository";
import { Effect } from "effect";

export function getCoverageMatrix(userId: string, codebaseId?: string) {
    return Effect.all({
        coverage: getCoverage(userId, codebaseId),
        matrix: getChunkCoverageMatrix(userId, codebaseId)
    }).pipe(
        Effect.map(({ coverage, matrix }) => ({
            ...coverage,
            matrix
        }))
    );
}

export function getCoverage(userId: string, codebaseId?: string) {
    return getChunkCoverage(userId, codebaseId).pipe(
        Effect.map(rows => {
            const covered: { id: string; title: string; requirementCount: number }[] = [];
            const uncovered: { id: string; title: string }[] = [];

            for (const row of rows) {
                const count = Number(row.requirementCount);
                if (count > 0) {
                    covered.push({ id: row.id, title: row.title, requirementCount: count });
                } else {
                    uncovered.push({ id: row.id, title: row.title });
                }
            }

            const total = rows.length;
            const coveredCount = covered.length;
            const uncoveredCount = uncovered.length;
            const percentage = total > 0 ? Math.round((coveredCount / total) * 100) : 0;

            return {
                covered,
                uncovered,
                stats: { total, covered: coveredCount, uncovered: uncoveredCount, percentage }
            };
        })
    );
}
