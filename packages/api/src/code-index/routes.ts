// packages/api/src/code-index/routes.ts
import { Effect } from "effect";
import { Elysia, t } from "elysia";

import { indexDirectory, syncIndexToGraph } from "./service";

export const codeIndexRoutes = new Elysia({ prefix: "/code-index" })
    .post(
        "/scan",
        async ctx => {
            const { path } = ctx.body;
            const files = await Effect.runPromise(indexDirectory(path));
            const result = await Effect.runPromise(syncIndexToGraph(files));
            return { indexed: files.length, synced: result.synced };
        },
        {
            body: t.Object({ path: t.String() })
        }
    )
    .get("/status", async () => {
        return { available: true };
    });
