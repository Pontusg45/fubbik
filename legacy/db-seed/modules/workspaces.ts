import { eq } from "drizzle-orm";

import { workspace, workspaceSpace } from "../../schema/workspace";
import type { SeedContext } from "../context";
import { uuid } from "../factories";

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
        const spaceId = ctx.ids.codebases[cname];
        if (spaceId) pairs.push([id, spaceId]);
    }
    if (pairs.length > 0) {
        await ctx.db.insert(workspaceSpace).values(pairs.map(([workspaceId, spaceId]) => ({ workspaceId, spaceId })));
    }
    ctx.counters["workspaces"] = 1;
}

export async function reset(ctx: SeedContext): Promise<void> {
    await ctx.db.delete(workspace).where(eq(workspace.userId, ctx.userId));
}
