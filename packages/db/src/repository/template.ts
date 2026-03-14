import { and, eq, or } from "drizzle-orm";
import { Effect } from "effect";

import { DatabaseError } from "../errors";
import { db } from "../index";
import { chunkTemplate } from "../schema/template";

export function listTemplates(userId: string) {
    return Effect.tryPromise({
        try: () =>
            db
                .select()
                .from(chunkTemplate)
                .where(or(eq(chunkTemplate.isBuiltIn, true), eq(chunkTemplate.userId, userId))),
        catch: cause => new DatabaseError({ cause })
    });
}

export function getTemplateById(id: string) {
    return Effect.tryPromise({
        try: async () => {
            const [found] = await db.select().from(chunkTemplate).where(eq(chunkTemplate.id, id));
            return found ?? null;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function createTemplate(params: {
    id: string;
    name: string;
    description?: string | null;
    type: string;
    content: string;
    userId: string;
}) {
    return Effect.tryPromise({
        try: async () => {
            const [created] = await db
                .insert(chunkTemplate)
                .values({
                    id: params.id,
                    name: params.name,
                    description: params.description ?? null,
                    type: params.type,
                    content: params.content,
                    isBuiltIn: false,
                    userId: params.userId
                })
                .returning();
            return created;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function updateTemplate(
    id: string,
    userId: string,
    params: { name?: string; description?: string | null; type?: string; content?: string }
) {
    return Effect.tryPromise({
        try: async () => {
            const [updated] = await db
                .update(chunkTemplate)
                .set({
                    ...(params.name !== undefined && { name: params.name }),
                    ...(params.description !== undefined && { description: params.description }),
                    ...(params.type !== undefined && { type: params.type }),
                    ...(params.content !== undefined && { content: params.content })
                })
                .where(and(eq(chunkTemplate.id, id), eq(chunkTemplate.userId, userId)))
                .returning();
            return updated ?? null;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function deleteTemplate(id: string, userId: string) {
    return Effect.tryPromise({
        try: async () => {
            const [deleted] = await db
                .delete(chunkTemplate)
                .where(
                    and(
                        eq(chunkTemplate.id, id),
                        eq(chunkTemplate.userId, userId),
                        eq(chunkTemplate.isBuiltIn, false)
                    )
                )
                .returning();
            return deleted ?? null;
        },
        catch: cause => new DatabaseError({ cause })
    });
}
