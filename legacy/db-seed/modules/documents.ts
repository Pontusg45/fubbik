import { eq } from "drizzle-orm";

import { document } from "../../schema/document";
import type { SeedContext } from "../context";
import { uuid } from "../factories";

export async function seed(ctx: SeedContext): Promise<void> {
    const spaceId = ctx.ids.codebases["fubbik"];
    if (!spaceId) throw new Error("documents needs fubbik space");

    const docs = [
        {
            name: "docs-getting-started",
            title: "Getting Started",
            sourcePath: "docs/guide/getting-started.md",
            description: "Install, seed, run dev — the 60-second path to a live fubbik"
        },
        {
            name: "docs-architecture",
            title: "Architecture",
            sourcePath: "docs/guide/architecture.md",
            description: "Monorepo layout, request flow, error handling"
        },
        {
            name: "docs-chunks",
            title: "Chunks",
            sourcePath: "docs/guide/chunks.md",
            description: "How chunks work, what types exist, how to capture one"
        }
    ];

    for (const d of docs) {
        const id = uuid();
        await ctx.db.insert(document).values({
            id,
            title: d.title,
            sourcePath: d.sourcePath,
            contentHash: "seed-placeholder",
            description: d.description,
            userId: ctx.userId,
            spaceId
        });
        ctx.ids.documents[d.name] = id;
    }
    ctx.counters["documents"] = docs.length;
}

export async function reset(ctx: SeedContext): Promise<void> {
    await ctx.db.delete(document).where(eq(document.userId, ctx.userId));
}
