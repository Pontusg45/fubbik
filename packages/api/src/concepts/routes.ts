import { Effect } from "effect";
import { Elysia } from "elysia";

import { detectEmergentConcepts, listConcepts } from "./service";

export const conceptRoutes = new Elysia({ prefix: "/concepts" })
    .get("/", async () => {
        const concepts = await Effect.runPromise(listConcepts());
        return { concepts };
    })
    .post("/detect", async () => {
        const result = await Effect.runPromise(detectEmergentConcepts());
        return result;
    });
