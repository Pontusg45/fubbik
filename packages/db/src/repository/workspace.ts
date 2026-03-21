import { and, eq } from "drizzle-orm";
import { Effect } from "effect";

import { DatabaseError } from "../errors";
import { db } from "../index";
import { codebase } from "../schema/codebase";
import { workspace, workspaceCodebase } from "../schema/workspace";

// ── Workspaces ────────────────────────────────────────────────────

export interface CreateWorkspaceParams {
    id: string;
    name: string;
    description?: string;
    userId: string;
}

export function createWorkspace(params: CreateWorkspaceParams) {
    return Effect.tryPromise({
        try: async () => {
            const [created] = await db.insert(workspace).values(params).returning();
            return created!;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function getWorkspaceById(id: string, userId?: string) {
    return Effect.tryPromise({
        try: async () => {
            const conditions = [eq(workspace.id, id)];
            if (userId) conditions.push(eq(workspace.userId, userId));
            const [found] = await db
                .select()
                .from(workspace)
                .where(and(...conditions));
            return found ?? null;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function listWorkspaces(userId: string) {
    return Effect.tryPromise({
        try: () => db.select().from(workspace).where(eq(workspace.userId, userId)),
        catch: cause => new DatabaseError({ cause })
    });
}

export interface UpdateWorkspaceParams {
    name?: string;
    description?: string | null;
}

export function updateWorkspace(id: string, userId: string, params: UpdateWorkspaceParams) {
    return Effect.tryPromise({
        try: async () => {
            const setClause: Record<string, unknown> = {};
            if (params.name !== undefined) setClause.name = params.name;
            if (params.description !== undefined) setClause.description = params.description;

            if (Object.keys(setClause).length === 0) {
                const [found] = await db
                    .select()
                    .from(workspace)
                    .where(and(eq(workspace.id, id), eq(workspace.userId, userId)));
                return found ?? null;
            }

            const [updated] = await db
                .update(workspace)
                .set(setClause)
                .where(and(eq(workspace.id, id), eq(workspace.userId, userId)))
                .returning();
            return updated ?? null;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function deleteWorkspace(id: string, userId: string) {
    return Effect.tryPromise({
        try: async () => {
            const [deleted] = await db
                .delete(workspace)
                .where(and(eq(workspace.id, id), eq(workspace.userId, userId)))
                .returning();
            return deleted ?? null;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

// ── Workspace Codebases ───────────────────────────────────────────

export function getCodebasesForWorkspace(workspaceId: string) {
    return Effect.tryPromise({
        try: () =>
            db
                .select({
                    id: codebase.id,
                    name: codebase.name,
                    remoteUrl: codebase.remoteUrl,
                    localPaths: codebase.localPaths
                })
                .from(workspaceCodebase)
                .innerJoin(codebase, eq(workspaceCodebase.codebaseId, codebase.id))
                .where(eq(workspaceCodebase.workspaceId, workspaceId)),
        catch: cause => new DatabaseError({ cause })
    });
}

export function addCodebaseToWorkspace(workspaceId: string, codebaseId: string) {
    return Effect.tryPromise({
        try: async () => {
            const [created] = await db
                .insert(workspaceCodebase)
                .values({ workspaceId, codebaseId })
                .onConflictDoNothing()
                .returning();
            return created ?? { workspaceId, codebaseId };
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function removeCodebaseFromWorkspace(workspaceId: string, codebaseId: string) {
    return Effect.tryPromise({
        try: async () => {
            const [deleted] = await db
                .delete(workspaceCodebase)
                .where(
                    and(
                        eq(workspaceCodebase.workspaceId, workspaceId),
                        eq(workspaceCodebase.codebaseId, codebaseId)
                    )
                )
                .returning();
            return deleted ?? null;
        },
        catch: cause => new DatabaseError({ cause })
    });
}
