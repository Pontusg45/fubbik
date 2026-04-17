import { eq } from "drizzle-orm";

import { useCase } from "../../schema/use-case";
import { uuid } from "../factories";
import type { SeedContext } from "../context";

export async function seed(ctx: SeedContext): Promise<void> {
    const rows = [
        {
            id: uuid(),
            userId: ctx.userId,
            codebaseId: ctx.ids.codebases["fubbik"],
            name: "New team member onboarding",
            description: "A dev joins the fubbik team and wants to understand the architecture in a day."
        },
        {
            id: uuid(),
            userId: ctx.userId,
            codebaseId: ctx.ids.codebases["fubbik"],
            name: "Capture decisions near the code",
            description: "An engineer makes an architecture decision and wants a persistent, queryable record linked to the relevant files."
        }
    ];
    await ctx.db.insert(useCase).values(rows);
    ctx.ids.useCases["onboarding"] = rows[0]!.id;
    ctx.ids.useCases["decisions"] = rows[1]!.id;
    ctx.counters["use_cases"] = rows.length;
}

export async function reset(ctx: SeedContext): Promise<void> {
    await ctx.db.delete(useCase).where(eq(useCase.userId, ctx.userId));
}
