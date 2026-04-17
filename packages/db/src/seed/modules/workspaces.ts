import { eq } from "drizzle-orm";

import { workspace, workspaceCodebase } from "../../schema/workspace";
import { uuid } from "../factories";
import type { SeedContext } from "../context";

export async function seed(ctx: SeedContext): Promise<void> {
    const id = uuid();
    await ctx.db.insert(workspace).values({
        id,
        name: "fubbik-platform",
        description: "Main fubbik product — code + docs grouped together",
        userId: ctx.userId
    });
    ctx.ids.workspaces["platform"] = id;

    const pairs: Array<[string, string]> = [];
    for (const cname of Object.keys(ctx.ids.codebases)) {
        pairs.push([id, ctx.ids.codebases[cname]!]);
    }
    if (pairs.length > 0) {
        await ctx.db.insert(workspaceCodebase).values(
            pairs.map(([workspaceId, codebaseId]) => ({ workspaceId, codebaseId }))
        );
    }
    ctx.counters["workspaces"] = 1;
}

export async function reset(ctx: SeedContext): Promise<void> {
    await ctx.db.delete(workspace).where(eq(workspace.userId, ctx.userId));
}
