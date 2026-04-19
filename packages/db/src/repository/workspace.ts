import { and, eq } from "drizzle-orm";

import { db, dbEffect } from "../index";
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
    return dbEffect(async () => {
            const [created] = await db.insert(workspace).values(params).returning();
            return created!;
        });
}

export function getWorkspaceById(id: string, userId?: string) {
    return dbEffect(async () => {
            const conditions = [eq(workspace.id, id)];
            if (userId) conditions.push(eq(workspace.userId, userId));
            const [found] = await db
                .select()
                .from(workspace)
                .where(and(...conditions));
            return found ?? null;
        });
}

export function listWorkspaces(userId: string) {
    return dbEffect(() => db.select().from(workspace).where(eq(workspace.userId, userId)));
}

export interface UpdateWorkspaceParams {
    name?: string;
    description?: string | null;
}

export function updateWorkspace(id: string, userId: string, params: UpdateWorkspaceParams) {
    return dbEffect(async () => {
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
        });
}

export function deleteWorkspace(id: string, userId: string) {
    return dbEffect(async () => {
            const [deleted] = await db
                .delete(workspace)
                .where(and(eq(workspace.id, id), eq(workspace.userId, userId)))
                .returning();
            return deleted ?? null;
        });
}

// ── Workspace Codebases ───────────────────────────────────────────

export function getCodebasesForWorkspace(workspaceId: string) {
    return dbEffect(() =>
            db
                .select({
                    id: codebase.id,
                    name: codebase.name,
                    remoteUrl: codebase.remoteUrl,
                    localPaths: codebase.localPaths
                })
                .from(workspaceCodebase)
                .innerJoin(codebase, eq(workspaceCodebase.codebaseId, codebase.id))
                .where(eq(workspaceCodebase.workspaceId, workspaceId)));
}

export function addCodebaseToWorkspace(workspaceId: string, codebaseId: string) {
    return dbEffect(async () => {
            const [created] = await db
                .insert(workspaceCodebase)
                .values({ workspaceId, codebaseId })
                .onConflictDoNothing()
                .returning();
            return created ?? { workspaceId, codebaseId };
        });
}

export function removeCodebaseFromWorkspace(workspaceId: string, codebaseId: string) {
    return dbEffect(async () => {
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
        });
}
