import { and, eq, inArray, sql } from "drizzle-orm";
import { Effect } from "effect";

import { cypherVoid, isAgeAvailable } from "../age/client";
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
    return Effect.tryPromise({
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

            if (await isAgeAvailable()) {
                const { ancestors: ancestorIds, descendants: descendantIds, edges } =
                    await Effect.runPromise(ageGetTransitiveDeps(requirementId));
                const [ancestors, descendants] = await Promise.all([
                    fetchById(ancestorIds),
                    fetchById(descendantIds)
                ]);
                return { ancestors, descendants, edges };
            }

            // CTE fallback when AGE is unavailable
            const ancestorResult = await db.execute(sql`
                WITH RECURSIVE chain AS (
                    SELECT depends_on_id AS id FROM requirement_dependency WHERE requirement_id = ${requirementId}
                    UNION
                    SELECT rd.depends_on_id FROM requirement_dependency rd
                    INNER JOIN chain c ON rd.requirement_id = c.id
                )
                SELECT id FROM chain
            `);
            const ancestorIds = ancestorResult.rows.map((r: any) => r.id as string);

            const descendantResult = await db.execute(sql`
                WITH RECURSIVE chain AS (
                    SELECT requirement_id AS id FROM requirement_dependency WHERE depends_on_id = ${requirementId}
                    UNION
                    SELECT rd.requirement_id FROM requirement_dependency rd
                    INNER JOIN chain c ON rd.depends_on_id = c.id
                )
                SELECT id FROM chain
            `);
            const descendantIds = descendantResult.rows.map((r: any) => r.id as string);

            const edgeResult = await db
                .select({
                    source: requirementDependency.requirementId,
                    target: requirementDependency.dependsOnId
                })
                .from(requirementDependency)
                .where(
                    inArray(requirementDependency.requirementId, [requirementId, ...ancestorIds, ...descendantIds].length > 0
                        ? [requirementId, ...ancestorIds, ...descendantIds]
                        : [requirementId])
                );

            const [ancestors, descendants] = await Promise.all([
                fetchById(ancestorIds),
                fetchById(descendantIds)
            ]);

            return { ancestors, descendants, edges: edgeResult };
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function checkCircularDependency(requirementId: string, dependsOnId: string) {
    return Effect.tryPromise({
        try: async () => {
            if (await isAgeAvailable()) {
                return Effect.runPromise(ageCheckCircular(requirementId, dependsOnId));
            }
            // CTE fallback when AGE is unavailable
            const result = await db.execute(sql`
                WITH RECURSIVE chain AS (
                    SELECT depends_on_id AS id FROM requirement_dependency WHERE requirement_id = ${dependsOnId}
                    UNION
                    SELECT rd.depends_on_id FROM requirement_dependency rd
                    INNER JOIN chain c ON rd.requirement_id = c.id
                )
                SELECT 1 FROM chain WHERE id = ${requirementId} LIMIT 1
            `);
            return result.rows.length > 0;
        },
        catch: cause => new DatabaseError({ cause })
    });
}
