// packages/db/src/age/client.ts
import { sql } from "drizzle-orm";
import { Effect } from "effect";

import { DatabaseError } from "../errors";
import { db } from "../index";

export function initAge() {
    return Effect.tryPromise({
        try: async () => {
            await db.execute(sql`LOAD 'age'`);
            await db.execute(sql.raw(`SET search_path = ag_catalog, "$user", public`));
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function cypher(query: string, returnType = "v agtype") {
    return Effect.tryPromise({
        try: async () => {
            const result = await db.execute(
                sql.raw(`SELECT * FROM cypher('knowledge', $$ ${query} $$) AS (${returnType})`)
            );
            return result.rows;
        },
        catch: cause => new DatabaseError({ cause })
    });
}

export function cypherVoid(query: string) {
    return Effect.tryPromise({
        try: async () => {
            await db.execute(
                sql.raw(`SELECT * FROM cypher('knowledge', $$ ${query} $$) AS (v agtype)`)
            );
        },
        catch: cause => new DatabaseError({ cause })
    });
}
