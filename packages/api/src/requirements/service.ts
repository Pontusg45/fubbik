import {
    createRequirement as createRequirementRepo,
    getRequirementById,
    listRequirements as listRequirementsRepo,
    updateRequirement as updateRequirementRepo,
    deleteRequirement as deleteRequirementRepo,
    updateRequirementStatus,
    setRequirementChunks,
    getChunksForRequirement,
    getRequirementStats,
    bulkUpdateRequirements,
    bulkDeleteRequirements,
    getChunkById,
    listVocabulary
} from "@fubbik/db/repository";
import { Effect } from "effect";

import { NotFoundError, StepValidationError } from "../errors";
import { parseStepText, type VocabEntry, type VocabularyWarning } from "../vocabulary/parser";
import { validateSteps } from "./validator";
import { crossReferenceSteps, type CrossRefWarning } from "./cross-ref";
import { toGherkin, toVitest, toMarkdown } from "./export";

interface StepVocabularyWarning extends VocabularyWarning {
    step: number;
}

function getVocabularyWarnings(
    steps: Array<{ text: string }>,
    codebaseId: string | undefined | null
): Effect.Effect<StepVocabularyWarning[], never> {
    if (!codebaseId) return Effect.succeed([]);

    return listVocabulary(codebaseId).pipe(
        Effect.map(vocab => {
            const vocabEntries: VocabEntry[] = vocab.map(v => ({
                word: v.word,
                category: v.category,
                expects: v.expects
            }));
            const allWarnings: StepVocabularyWarning[] = [];
            for (let i = 0; i < steps.length; i++) {
                const result = parseStepText(steps[i]!.text, vocabEntries);
                for (const w of result.warnings) {
                    allWarnings.push({ ...w, step: i });
                }
            }
            return allWarnings;
        }),
        Effect.catchAll(() => Effect.succeed([] as StepVocabularyWarning[]))
    );
}

export function listRequirements(
    userId: string,
    query: {
        codebaseId?: string;
        useCaseId?: string;
        search?: string;
        status?: string;
        priority?: string;
        origin?: string;
        reviewStatus?: string;
        limit?: string;
        offset?: string;
    }
) {
    const limit = Math.min(Number(query.limit ?? 50), 100);
    const offset = Number(query.offset ?? 0);

    return listRequirementsRepo({
        userId,
        codebaseId: query.codebaseId,
        useCaseId: query.useCaseId,
        search: query.search,
        status: query.status,
        priority: query.priority,
        origin: query.origin,
        reviewStatus: query.reviewStatus,
        limit,
        offset
    });
}

export function getRequirement(id: string, userId: string) {
    return getRequirementById(id, userId).pipe(
        Effect.filterOrFail(
            (req): req is NonNullable<typeof req> => req !== null,
            () => new NotFoundError({ resource: "Requirement" })
        ),
        Effect.flatMap(req =>
            getChunksForRequirement(id).pipe(
                Effect.map(chunks => ({ ...req, chunks }))
            )
        )
    );
}

export function createRequirement(
    userId: string,
    body: {
        title: string;
        description?: string;
        steps: Array<{ keyword: "given" | "when" | "then" | "and" | "but"; text: string; params?: Record<string, string> }>;
        priority?: string;
        codebaseId?: string;
        useCaseId?: string;
        origin?: string;
    }
) {
    return Effect.gen(function* () {
        const errors = validateSteps(body.steps);
        if (errors.length > 0) {
            return yield* Effect.fail(new StepValidationError({ errors }));
        }

        const origin = body.origin ?? "human";
        const id = crypto.randomUUID();
        const requirement = yield* createRequirementRepo({
            id,
            title: body.title,
            description: body.description,
            steps: body.steps,
            priority: body.priority,
            codebaseId: body.codebaseId,
            useCaseId: body.useCaseId,
            userId,
            origin,
            reviewStatus: origin === "ai" ? "draft" : "approved"
        });
        const warnings = yield* crossReferenceSteps(body.steps, userId);
        const vocabularyWarnings = yield* getVocabularyWarnings(body.steps, body.codebaseId);
        return { requirement, warnings, vocabularyWarnings };
    });
}

export function updateRequirement(
    id: string,
    userId: string,
    body: {
        title?: string;
        description?: string | null;
        steps?: Array<{ keyword: "given" | "when" | "then" | "and" | "but"; text: string; params?: Record<string, string> }>;
        priority?: string | null;
        codebaseId?: string | null;
        useCaseId?: string | null;
        origin?: string;
        reviewStatus?: string;
    }
) {
    return Effect.gen(function* () {
        const existing = yield* getRequirementById(id, userId);
        if (!existing) return yield* Effect.fail(new NotFoundError({ resource: "Requirement" }));

        if (body.steps) {
            const errors = validateSteps(body.steps);
            if (errors.length > 0) {
                return yield* Effect.fail(new StepValidationError({ errors }));
            }
        }

        const repoBody: Record<string, unknown> = { ...body };
        if (body.reviewStatus !== undefined) {
            repoBody.reviewedBy = userId;
            repoBody.reviewedAt = new Date();
        }

        const requirement = yield* updateRequirementRepo(id, userId, repoBody as Parameters<typeof updateRequirementRepo>[2]);
        if (!requirement) return yield* Effect.fail(new NotFoundError({ resource: "Requirement" }));

        const warnings = body.steps
            ? yield* crossReferenceSteps(body.steps, userId)
            : ([] as CrossRefWarning[]);

        const codebaseId = body.codebaseId !== undefined ? body.codebaseId : existing.codebaseId;
        const vocabularyWarnings = body.steps
            ? yield* getVocabularyWarnings(body.steps, codebaseId)
            : ([] as StepVocabularyWarning[]);

        return { requirement, warnings, vocabularyWarnings };
    });
}

export function deleteRequirement(id: string, userId: string) {
    return getRequirementById(id, userId).pipe(
        Effect.filterOrFail(
            (existing): existing is NonNullable<typeof existing> => existing !== null,
            () => new NotFoundError({ resource: "Requirement" })
        ),
        Effect.flatMap(() => deleteRequirementRepo(id, userId))
    );
}

export function updateStatus(id: string, userId: string, status: string) {
    return updateRequirementStatus(id, userId, status).pipe(
        Effect.filterOrFail(
            (updated): updated is NonNullable<typeof updated> => updated !== null,
            () => new NotFoundError({ resource: "Requirement" })
        )
    );
}

export function setChunks(requirementId: string, userId: string, chunkIds: string[]) {
    return getRequirementById(requirementId, userId).pipe(
        Effect.filterOrFail(
            (req): req is NonNullable<typeof req> => req !== null,
            () => new NotFoundError({ resource: "Requirement" })
        ),
        Effect.flatMap(() => {
            const verifications = chunkIds.map(chunkId =>
                getChunkById(chunkId, userId).pipe(
                    Effect.filterOrFail(
                        (chunk): chunk is NonNullable<typeof chunk> => chunk !== null && chunk !== undefined,
                        () => new NotFoundError({ resource: `Chunk ${chunkId}` })
                    )
                )
            );

            return Effect.all(verifications).pipe(
                Effect.flatMap(() => setRequirementChunks(requirementId, chunkIds))
            );
        })
    );
}

export function getStats(userId: string, codebaseId?: string) {
    return getRequirementStats(userId, codebaseId);
}

export function bulkAction(
    userId: string,
    body: {
        ids: string[];
        action: "set_status" | "set_use_case" | "delete";
        status?: string;
        useCaseId?: string | null;
    }
) {
    switch (body.action) {
        case "set_status":
            return bulkUpdateRequirements(body.ids, userId, { status: body.status });
        case "set_use_case":
            return bulkUpdateRequirements(body.ids, userId, { useCaseId: body.useCaseId });
        case "delete":
            return bulkDeleteRequirements(body.ids, userId);
    }
}

function exportOne(
    title: string,
    steps: Array<{ keyword: "given" | "when" | "then" | "and" | "but"; text: string; params?: Record<string, string> }>,
    format: string
): string {
    switch (format) {
        case "gherkin":
            return toGherkin(title, steps);
        case "vitest":
            return toVitest(title, steps);
        case "markdown":
            return toMarkdown(title, steps);
        default:
            return toMarkdown(title, steps);
    }
}

export function exportRequirement(id: string, userId: string, format: string) {
    return getRequirementById(id, userId).pipe(
        Effect.filterOrFail(
            (req): req is NonNullable<typeof req> => req !== null,
            () => new NotFoundError({ resource: "Requirement" })
        ),
        Effect.map(req => exportOne(req.title, req.steps, format))
    );
}

export function exportAll(
    userId: string,
    query: { codebaseId?: string; format: string }
) {
    return listRequirementsRepo({
        userId,
        codebaseId: query.codebaseId,
        limit: 10000,
        offset: 0
    }).pipe(
        Effect.map(({ requirements }) => {
            const separator = query.format === "markdown" ? "\n\n---\n\n" : "\n\n";
            return requirements
                .map(r => exportOne(r.title, r.steps, query.format))
                .join(separator);
        })
    );
}
