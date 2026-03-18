import {
    listUseCases,
    listRequirementsByUseCase,
    listRequirements as listRequirementsRepo,
    getChunkCoverage,
    getOrphanChunks,
    getStaleChunks,
    getThinChunks,
    listChunks
} from "@fubbik/db/repository";
import { Effect } from "effect";

export function getSuggestContext(
    userId: string,
    query: { focus?: string; codebaseId?: string }
) {
    return Effect.gen(function* () {
        const { focus, codebaseId } = query;

        // 1. Fetch use cases with their requirements
        const useCases = yield* listUseCases(userId, codebaseId);

        const useCasesWithRequirements: Array<{
            id: string;
            name: string;
            description: string | null;
            requirements: Array<{ id: string; title: string; status: string; priority: string | null }>;
        }> = [];

        for (const uc of useCases) {
            const reqs = yield* listRequirementsByUseCase(uc.id, userId);
            const filtered = focus
                ? reqs.filter(r => r.title.toLowerCase().includes(focus.toLowerCase()))
                : reqs;
            if (focus && filtered.length === 0) continue;
            useCasesWithRequirements.push({
                id: uc.id,
                name: uc.name,
                description: uc.description ?? null,
                requirements: filtered.map(r => ({
                    id: r.id,
                    title: r.title,
                    status: r.status,
                    priority: r.priority ?? null
                }))
            });
        }

        // Also include ungrouped requirements (useCaseId is null)
        const ungroupedResult = yield* listRequirementsRepo({
            userId,
            codebaseId,
            limit: 200,
            offset: 0
        });
        const ungrouped = ungroupedResult.requirements.filter(r => r.useCaseId === null);
        const filteredUngrouped = focus
            ? ungrouped.filter(r => r.title.toLowerCase().includes(focus.toLowerCase()))
            : ungrouped;

        // 2. Fetch coverage gaps
        const coverageChunks = yield* getChunkCoverage(userId, codebaseId);
        let uncovered = coverageChunks.filter(c => Number(c.requirementCount) === 0);
        if (focus) {
            uncovered = uncovered.filter(c => c.title.toLowerCase().includes(focus.toLowerCase()));
        }
        const coverageGaps = uncovered
            .slice(0, focus ? 20 : 10)
            .map(c => ({ id: c.id, title: c.title }));

        // 3. Fetch health issue counts
        const [orphans, stale, thin] = yield* Effect.all([
            getOrphanChunks(userId, codebaseId),
            getStaleChunks(userId, codebaseId),
            getThinChunks(userId, codebaseId)
        ]);
        const healthIssueCounts = {
            orphan: orphans.count,
            stale: stale.count,
            thin: thin.count
        };

        // 4. Fetch relevant chunks (only when focus provided)
        let relevantChunks: Array<{ id: string; title: string; content: string; type: string }> = [];
        if (focus) {
            const chunksResult = yield* listChunks({
                userId,
                codebaseId,
                search: focus,
                limit: 20,
                offset: 0
            });
            relevantChunks = chunksResult.chunks.map(c => ({
                id: c.id,
                title: c.title,
                content: c.content.slice(0, 300),
                type: c.type
            }));
        }

        return {
            useCases: useCasesWithRequirements,
            ungroupedRequirements: filteredUngrouped.map(r => ({
                id: r.id,
                title: r.title,
                status: r.status,
                priority: r.priority ?? null
            })),
            coverageGaps,
            healthIssueCounts,
            relevantChunks
        };
    });
}
