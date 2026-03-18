import {
    createRequirement,
    createUseCase,
    getUseCaseByName
} from "@fubbik/db/repository";
import { Effect } from "effect";
import { StepValidationError } from "../errors";
import { validateSteps } from "./validator";

import type { RequirementStep } from "@fubbik/db/schema/requirement";

interface BatchRequirement {
    title: string;
    description?: string;
    steps: RequirementStep[];
    priority?: string;
    useCaseId?: string;
    useCaseName?: string;
    parentUseCaseName?: string;
}

interface BatchBody {
    requirements: BatchRequirement[];
    codebaseId?: string;
}

export function batchCreateRequirements(userId: string, body: BatchBody) {
    return Effect.gen(function* () {
        // 1. Validate all steps upfront
        const allStepErrors: Array<{ index: number; step: number; error: string }> = [];
        for (let i = 0; i < body.requirements.length; i++) {
            const req = body.requirements[i]!;
            const errors = validateSteps(req.steps);
            for (const err of errors) {
                allStepErrors.push({ index: i, ...err });
            }
        }
        if (allStepErrors.length > 0) {
            return yield* Effect.fail(new StepValidationError({ errors: allStepErrors }));
        }

        // 2. Resolve use cases with caching
        const nameToId = new Map<string, string>();
        const useCasesCreated: Array<{ id: string; name: string; parentId?: string }> = [];

        const resolveUseCaseName = (name: string, parentId?: string) =>
            Effect.gen(function* () {
                const cached = nameToId.get(name);
                if (cached) return cached;

                const existing = yield* getUseCaseByName(userId, name);
                if (existing) {
                    nameToId.set(name, existing.id);
                    return existing.id;
                }

                const id = crypto.randomUUID();
                yield* createUseCase({
                    id,
                    name,
                    userId,
                    codebaseId: body.codebaseId,
                    parentId
                });
                nameToId.set(name, id);
                useCasesCreated.push({ id, name, parentId });
                return id;
            });

        // 3. Create requirements
        const created: Array<{ id: string; title: string; useCaseId?: string | null }> = [];

        for (const req of body.requirements) {
            let useCaseId = req.useCaseId;

            if (!useCaseId && req.useCaseName) {
                let parentId: string | undefined;
                if (req.parentUseCaseName) {
                    parentId = yield* resolveUseCaseName(req.parentUseCaseName);
                }
                useCaseId = yield* resolveUseCaseName(req.useCaseName, parentId);
            }

            const id = crypto.randomUUID();
            const result = yield* createRequirement({
                id,
                title: req.title,
                description: req.description,
                steps: req.steps,
                priority: req.priority,
                codebaseId: body.codebaseId,
                useCaseId,
                userId,
                origin: "ai",
                reviewStatus: "draft"
            });

            created.push({ id: result.id, title: result.title, useCaseId: result.useCaseId });
        }

        return {
            created: created.length,
            requirements: created,
            useCasesCreated
        };
    });
}
