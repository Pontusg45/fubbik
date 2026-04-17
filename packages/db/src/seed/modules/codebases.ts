/**
 * Codebases module: seeds the fubbik codebase and (optionally) a docs codebase.
 * Registers each codebase's id under ctx.ids.codebases by name so later modules
 * can reference "fubbik" instead of a UUID.
 */

import { eq } from "drizzle-orm";

import { codebase } from "../../schema/codebase";
import { makeCodebase } from "../factories";
import type { SeedContext } from "../context";

export async function seed(ctx: SeedContext): Promise<void> {
    const main = makeCodebase({
        id: "seed-codebase-fubbik",
        userId: ctx.userId,
        name: "fubbik",
        remoteUrl: "git@github.com:Pontusg45/fubbik.git",
        localPaths: ["/Users/pontus/projects/fubbik"]
    });
    await ctx.db.insert(codebase).values(main);
    ctx.ids.codebases["fubbik"] = main.id!;

    const docs = makeCodebase({
        id: "seed-codebase-docs",
        userId: ctx.userId,
        name: "fubbik-docs",
        remoteUrl: null,
        localPaths: []
    });
    await ctx.db.insert(codebase).values(docs);
    ctx.ids.codebases["docs"] = docs.id!;

    ctx.counters["codebases"] = 2;
}

export async function reset(ctx: SeedContext): Promise<void> {
    await ctx.db.delete(codebase).where(eq(codebase.userId, ctx.userId));
}
