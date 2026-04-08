import { and, eq, inArray } from "drizzle-orm";
import { Effect } from "effect";

import { cypherVoid } from "../age/client";
import { getTransitiveDeps as ageGetTransitiveDeps, checkCircular as ageCheckCircular } from "../age/query";
import { ensureVertex, createEdge } from "../age/sync";
import { DatabaseError } from "../errors";
import { db } from "../index";
import { requirement } from "../schema/requirement";
import { requirementDependency } from "../schema/requirement-dependency";

export function addDependency(requirementId: string, dependsOnId: string) {
    return Effect.tryPromise({
        try: async () => {
            const result = await db.insert(requirementDependency)
                .values({ requirementId, dependsOnId })
                .onConflictDoNothing()
                .returning();
            await Effect.runPromise(
                ensureVertex("requirement", requirementId).pipe(
                    Effect.flatMap(() => ensureVertex("requirement", dependsOnId)),
                    Effect.flatMap(() => createEdge("depends_on", "requirement", requirementId, "requirement", dependsOnId)),
                    Effect.catchAll(() => Effect.succeed(undefined))
                )
            );
            return result;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function removeDependency(requirementId: string, dependsOnId: string) {
    return Effect.tryPromise({
        try: async () => {
            const result = await db.delete(requirementDependency)
                .where(and(
                    eq(requirementDependency.requirementId, requirementId),
                    eq(requirementDependency.dependsOnId, dependsOnId)
                ));
            await Effect.runPromise(
                cypherVoid(
                    `MATCH (a:requirement {id: '${requirementId}'})-[e:depends_on]->(b:requirement {id: '${dependsOnId}'}) DELETE e`
                ).pipe(Effect.catchAll(() => Effect.succeed(undefined)))
            );
            return result;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function getDependencies(requirementId: string) {
    return Effect.tryPromise({
        try: async () => {
            const dependsOn = await db
                .select({
                    id: requirement.id,
                    title: requirement.title,
                    status: requirement.status,
                    priority: requirement.priority
                })
                .from(requirementDependency)
                .innerJoin(requirement, eq(requirementDependency.dependsOnId, requirement.id))
                .where(eq(requirementDependency.requirementId, requirementId));

            const dependedOnBy = await db
                .select({
                    id: requirement.id,
                    title: requirement.title,
                    status: requirement.status,
                    priority: requirement.priority
                })
                .from(requirementDependency)
                .innerJoin(requirement, eq(requirementDependency.requirementId, requirement.id))
                .where(eq(requirementDependency.dependsOnId, requirementId));

            return { dependsOn, dependedOnBy };
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function getTransitiveDependencies(requirementId: string) {
    return ageGetTransitiveDeps(requirementId).pipe(
        Effect.flatMap(({ ancestors: ancestorIds, descendants: descendantIds, edges }) =>
            Effect.tryPromise({
                try: async () => {
                    const fetchById = async (ids: string[]) => {
                        if (ids.length === 0) return [];
                        return db
                            .select({
                                id: requirement.id,
                                title: requirement.title,
                                status: requirement.status,
                                priority: requirement.priority
                            })
                            .from(requirement)
                            .where(inArray(requirement.id, ids));
                    };

                    const [ancestors, descendants] = await Promise.all([
                        fetchById(ancestorIds),
                        fetchById(descendantIds)
                    ]);

                    return { ancestors, descendants, edges };
                },
                catch: cause => new DatabaseError({ cause })
            })
        )
    );
}

export function checkCircularDependency(requirementId: string, dependsOnId: string) {
    return ageCheckCircular(requirementId, dependsOnId);
}
