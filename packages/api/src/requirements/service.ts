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
    getChunkById
} from "@fubbik/db/repository";
import { Effect } from "effect";

import { NotFoundError, StepValidationError } from "../errors";
import { validateSteps } from "./validator";
import { crossReferenceSteps } from "./cross-ref";
import { toGherkin, toVitest, toMarkdown } from "./export";

export function listRequirements(
    userId: string,
    query: {
        codebaseId?: string;
        status?: string;
        priority?: string;
        limit?: string;
        offset?: string;
    }
) {
    const limit = Math.min(Number(query.limit ?? 50), 100);
    const offset = Number(query.offset ?? 0);

    return listRequirementsRepo({
        userId,
        codebaseId: query.codebaseId,
        status: query.status,
        priority: query.priority,
        limit,
        offset
    });
}

export function getRequirement(id: string, userId: string) {
    return getRequirementById(id, userId).pipe(
        Effect.flatMap(req => {
            if (!req) return Effect.fail(new NotFoundError({ resource: "Requirement" }));
            return getChunksForRequirement(id).pipe(
                Effect.map(chunks => ({ ...req, chunks }))
            );
        })
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
    }
) {
    const errors = validateSteps(body.steps);
    if (errors.length > 0) {
        return Effect.fail(new StepValidationError({ errors }));
    }

    const id = crypto.randomUUID();
    return createRequirementRepo({
        id,
        title: body.title,
        description: body.description,
        steps: body.steps,
        priority: body.priority,
        codebaseId: body.codebaseId,
        userId
    }).pipe(
        Effect.flatMap(requirement =>
            crossReferenceSteps(body.steps, userId).pipe(
                Effect.map(warnings => ({ requirement, warnings }))
            )
        )
    );
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
    }
) {
    return getRequirementById(id, userId).pipe(
        Effect.flatMap(existing => {
            if (!existing) return Effect.fail(new NotFoundError({ resource: "Requirement" }));

            if (body.steps) {
                const errors = validateSteps(body.steps);
                if (errors.length > 0) {
                    return Effect.fail(new StepValidationError({ errors }));
                }
            }

            return updateRequirementRepo(id, userId, body).pipe(
                Effect.flatMap(requirement => {
                    if (!requirement) return Effect.fail(new NotFoundError({ resource: "Requirement" }));

                    if (body.steps) {
                        return crossReferenceSteps(body.steps, userId).pipe(
                            Effect.map(warnings => ({ requirement, warnings }))
                        );
                    }
                    return Effect.succeed({ requirement, warnings: [] });
                })
            );
        })
    );
}

export function deleteRequirement(id: string, userId: string) {
    return getRequirementById(id, userId).pipe(
        Effect.flatMap(existing => {
            if (!existing) return Effect.fail(new NotFoundError({ resource: "Requirement" }));
            return deleteRequirementRepo(id, userId);
        })
    );
}

export function updateStatus(id: string, userId: string, status: string) {
    return updateRequirementStatus(id, userId, status).pipe(
        Effect.flatMap(updated => {
            if (!updated) return Effect.fail(new NotFoundError({ resource: "Requirement" }));
            return Effect.succeed(updated);
        })
    );
}

export function setChunks(requirementId: string, userId: string, chunkIds: string[]) {
    return getRequirementById(requirementId, userId).pipe(
        Effect.flatMap(req => {
            if (!req) return Effect.fail(new NotFoundError({ resource: "Requirement" }));

            // Verify all chunk IDs belong to user
            const verifications = chunkIds.map(chunkId =>
                getChunkById(chunkId, userId).pipe(
                    Effect.flatMap(chunk => {
                        if (!chunk) return Effect.fail(new NotFoundError({ resource: `Chunk ${chunkId}` }));
                        return Effect.succeed(chunk);
                    })
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
        Effect.flatMap(req => {
            if (!req) return Effect.fail(new NotFoundError({ resource: "Requirement" }));
            return Effect.succeed(exportOne(req.title, req.steps, format));
        })
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
