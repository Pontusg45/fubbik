import { and, eq, sql } from "drizzle-orm";
import { Effect } from "effect";

import { DatabaseError } from "../errors";
import { db } from "../index";
import { requirement } from "../schema/requirement";
import { requirementDependency } from "../schema/requirement-dependency";

export function addDependency(requirementId: string, dependsOnId: string) {
    return Effect.tryPromise({
        try: () =>
            db.insert(requirementDependency)
                .values({ requirementId, dependsOnId })
                .onConflictDoNothing()
                .returning(),
        catch: cause => new DatabaseError({ cause })
    });
}

export function removeDependency(requirementId: string, dependsOnId: string) {
    return Effect.tryPromise({
        try: () =>
            db.delete(requirementDependency)
                .where(and(
                    eq(requirementDependency.requirementId, requirementId),
                    eq(requirementDependency.dependsOnId, dependsOnId)
                )),
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
            const ancestors = await db.execute(sql`
                WITH RECURSIVE ancestors AS (
                    SELECT depends_on_id AS id FROM requirement_dependency WHERE requirement_id = ${requirementId}
                    UNION
                    SELECT rd.depends_on_id FROM requirement_dependency rd
                    INNER JOIN ancestors a ON rd.requirement_id = a.id
                )
                SELECT r.id, r.title, r.status, r.priority
                FROM ancestors a
                INNER JOIN requirement r ON r.id = a.id
            `);

            const descendants = await db.execute(sql`
                WITH RECURSIVE descendants AS (
                    SELECT requirement_id AS id FROM requirement_dependency WHERE depends_on_id = ${requirementId}
                    UNION
                    SELECT rd.requirement_id FROM requirement_dependency rd
                    INNER JOIN descendants d ON rd.depends_on_id = d.id
                )
                SELECT r.id, r.title, r.status, r.priority
                FROM descendants d
                INNER JOIN requirement r ON r.id = d.id
            `);

            const allNodeIds = [
                requirementId,
                ...ancestors.rows.map((r: any) => r.id),
                ...descendants.rows.map((r: any) => r.id)
            ];

            const edges = allNodeIds.length > 0 ? await db.execute(sql`
                SELECT requirement_id AS source, depends_on_id AS target
                FROM requirement_dependency
                WHERE requirement_id = ANY(${allNodeIds})
                   OR depends_on_id = ANY(${allNodeIds})
            `) : { rows: [] };

            return { ancestors: ancestors.rows, descendants: descendants.rows, edges: edges.rows };
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function checkCircularDependency(requirementId: string, dependsOnId: string) {
    return Effect.tryPromise({
        try: async () => {
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
