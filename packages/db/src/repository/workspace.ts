import { and, eq } from "drizzle-orm";

import { db, dbEffect } from "../index";
import { space } from "../schema/space";
import { workspace, workspaceSpace } from "../schema/workspace";

// ── Workspaces (unchanged) ────────────────────────────────────────

export interface CreateWorkspaceParams {
    id: string;
    name: string;
    description?: string;
    userId: string;
}

export function createWorkspace(params: CreateWorkspaceParams) {
    return dbEffect(async () => {
        const [created] = await db.insert(workspace).values(params).returning();
        if (!created) throw new Error("createWorkspace: insert returned no row");
        return created;
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

// ── Workspace Spaces ──────────────────────────────────────────────

export function getSpacesForWorkspace(workspaceId: string) {
    return dbEffect(() =>
        db
            .select({
                id: space.id,
                name: space.name,
                kind: space.kind
            })
            .from(workspaceSpace)
            .innerJoin(space, eq(workspaceSpace.spaceId, space.id))
            .where(eq(workspaceSpace.workspaceId, workspaceId))
    );
}

export function addSpaceToWorkspace(workspaceId: string, spaceId: string) {
    return dbEffect(async () => {
        const [created] = await db.insert(workspaceSpace).values({ workspaceId, spaceId }).onConflictDoNothing().returning();
        return created ?? { workspaceId, spaceId };
    });
}

export function removeSpaceFromWorkspace(workspaceId: string, spaceId: string) {
    return dbEffect(async () => {
        const [deleted] = await db
            .delete(workspaceSpace)
            .where(and(eq(workspaceSpace.workspaceId, workspaceId), eq(workspaceSpace.spaceId, spaceId)))
            .returning();
        return deleted ?? null;
    });
}
