import { eq } from "drizzle-orm";

import { vocabularyEntry } from "../../schema/vocabulary";
import { uuid } from "../factories";
import type { SeedContext } from "../context";

export async function seed(ctx: SeedContext): Promise<void> {
    const codebaseId = ctx.ids.codebases["fubbik"];
    if (!codebaseId) throw new Error("vocabulary needs fubbik codebase");

    const entries = [
        { word: "chunk", category: "noun" },
        { word: "plan", category: "noun" },
        { word: "codebase", category: "noun" },
        { word: "tag", category: "noun" },
        { word: "capture", category: "verb", expects: ["chunk", "note"] },
        { word: "link", category: "verb", expects: ["chunk", "chunk"] },
        { word: "seed", category: "verb" },
        { word: "apply", category: "verb", expects: ["filter", "plan"] },
        { word: "must", category: "modal" },
        { word: "should", category: "modal" }
    ];

    await ctx.db.insert(vocabularyEntry).values(
        entries.map(e => ({
            id: uuid(),
            word: e.word,
            category: e.category,
            expects: e.expects ?? null,
            codebaseId,
            userId: ctx.userId
        }))
    );
    ctx.counters["vocabulary"] = entries.length;
}

export async function reset(ctx: SeedContext): Promise<void> {
    await ctx.db.delete(vocabularyEntry).where(eq(vocabularyEntry.userId, ctx.userId));
}
