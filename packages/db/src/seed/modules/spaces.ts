/**
 * Spaces module: seeds the fubbik space and (optionally) a docs space.
 * Registers each space's id under ctx.ids.codebases by name so later modules
 * can reference "fubbik" instead of a UUID.
 */

import { eq } from "drizzle-orm";

import { space } from "../../schema/space";
import { spaceCodeMetadata } from "../../schema/space-code-metadata";
import { uuid } from "../factories";
import type { SeedContext } from "../context";

export async function seed(ctx: SeedContext): Promise<void> {
    const fubbikId = "seed-codebase-fubbik";
    await ctx.db.insert(space).values({
        id: fubbikId,
        name: "fubbik",
        kind: "code",
        userId: ctx.userId
    });
    await ctx.db.insert(spaceCodeMetadata).values({
        spaceId: fubbikId,
        userId: ctx.userId,
        remoteUrl: "git@github.com:Pontusg45/fubbik.git",
        localPaths: ["/Users/pontus/projects/fubbik"]
    });
    ctx.ids.codebases["fubbik"] = fubbikId;

    const docsId = uuid();
    await ctx.db.insert(space).values({
        id: docsId,
        name: "fubbik-docs",
        kind: "code",
        userId: ctx.userId
    });
    await ctx.db.insert(spaceCodeMetadata).values({
        spaceId: docsId,
        userId: ctx.userId,
        remoteUrl: null,
        localPaths: []
    });
    ctx.ids.codebases["docs"] = docsId;

    ctx.counters["codebases"] = 2;
}

export async function reset(ctx: SeedContext): Promise<void> {
    await ctx.db.delete(space).where(eq(space.userId, ctx.userId));
}
