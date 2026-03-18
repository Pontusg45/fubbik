import {
    addDependency as addDependencyRepo,
    removeDependency as removeDependencyRepo,
    getDependencies as getDependenciesRepo,
    getTransitiveDependencies,
    checkCircularDependency,
    getRequirementById
} from "@fubbik/db/repository";
import { Effect } from "effect";
import { NotFoundError, ValidationError } from "../errors";

export function addDependency(requirementId: string, dependsOnId: string, userId: string) {
    return Effect.gen(function* () {
        const req = yield* getRequirementById(requirementId, userId);
        if (!req) return yield* Effect.fail(new NotFoundError({ resource: "Requirement" }));

        const dep = yield* getRequirementById(dependsOnId, userId);
        if (!dep) return yield* Effect.fail(new NotFoundError({ resource: "Dependency target" }));

        const wouldCycle = yield* checkCircularDependency(requirementId, dependsOnId);
        if (wouldCycle) return yield* Effect.fail(new ValidationError({ message: "Adding this dependency would create a circular reference" }));

        return yield* addDependencyRepo(requirementId, dependsOnId);
    });
}

export function removeDependency(requirementId: string, dependsOnId: string, userId: string) {
    return Effect.gen(function* () {
        const req = yield* getRequirementById(requirementId, userId);
        if (!req) return yield* Effect.fail(new NotFoundError({ resource: "Requirement" }));
        return yield* removeDependencyRepo(requirementId, dependsOnId);
    });
}

export function getDependencies(requirementId: string, userId: string) {
    return Effect.gen(function* () {
        const req = yield* getRequirementById(requirementId, userId);
        if (!req) return yield* Effect.fail(new NotFoundError({ resource: "Requirement" }));
        return yield* getDependenciesRepo(requirementId);
    });
}

export function getDependencyGraph(requirementId: string, userId: string) {
    return Effect.gen(function* () {
        const req = yield* getRequirementById(requirementId, userId);
        if (!req) return yield* Effect.fail(new NotFoundError({ resource: "Requirement" }));

        const { ancestors, descendants, edges } = yield* getTransitiveDependencies(requirementId);

        const allReqs = [
            { id: req.id, title: req.title, status: req.status, priority: req.priority, isCurrent: true },
            ...ancestors.map((r: any) => ({ id: r.id, title: r.title, status: r.status, priority: r.priority, isCurrent: false })),
            ...descendants.map((r: any) => ({ id: r.id, title: r.title, status: r.status, priority: r.priority, isCurrent: false }))
        ];

        const nodeMap = new Map(allReqs.map(r => [r.id, r]));
        const nodes = Array.from(nodeMap.values());

        return {
            nodes,
            edges: (edges as any[]).map((e: any) => ({
                source: e.source,
                target: e.target
            }))
        };
    });
}
