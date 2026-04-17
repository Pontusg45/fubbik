import { eq } from "drizzle-orm";

import { collection } from "../../schema/collection";
import { uuid } from "../factories";
import type { SeedContext } from "../context";

export async function seed(ctx: SeedContext): Promise<void> {
    await ctx.db.insert(collection).values([
        {
            id: uuid(),
            name: "Conventions",
            description: "Pinned conventions — always include in AI context",
            filter: { type: "convention" },
            userId: ctx.userId,
            codebaseId: ctx.ids.codebases["fubbik"] ?? null
        },
        {
            id: uuid(),
            name: "Self-documenting",
            description: "Chunks that describe fubbik itself",
            filter: { tags: "self-documenting" },
            userId: ctx.userId,
            codebaseId: ctx.ids.codebases["fubbik"] ?? null
        }
    ]);
    ctx.counters["collections"] = 2;
}

export async function reset(ctx: SeedContext): Promise<void> {
    await ctx.db.delete(collection).where(eq(collection.userId, ctx.userId));
}
