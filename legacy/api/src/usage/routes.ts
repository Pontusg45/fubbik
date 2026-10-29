import { Effect } from "effect";
import { Elysia, t } from "elysia";

import { aggregateCoReferences } from "./service";

export const usageRoutes = new Elysia({ prefix: "/usage" })
    .post("/aggregate", async () => {
        const result = await Effect.runPromise(aggregateCoReferences());
        return result;
    })
    .get(
        "/co-references",
        async ctx => {
            // Return co-referenced edges for a chunk from AGE
            const chunkId = ctx.query.chunkId;
            if (!chunkId) return { edges: [] };
            return { edges: [] }; // Placeholder — will be enriched when graph service is updated
        },
        { query: t.Object({ chunkId: t.Optional(t.String()) }) }
    );
