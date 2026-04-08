// packages/db/src/age/client.ts
import { sql } from "drizzle-orm";
import { Effect } from "effect";

import { DatabaseError } from "../errors";
import { db } from "../index";

let ageAvailable: boolean | null = null;

async function checkAgeAvailable(): Promise<boolean> {
    if (ageAvailable !== null) return ageAvailable;
    try {
        await db.execute(sql.raw(`SELECT 1 FROM ag_catalog.ag_graph LIMIT 0`));
        ageAvailable = true;
    } catch {
        ageAvailable = false;
    }
    return ageAvailable;
}

export function isAgeAvailable(): Promise<boolean> {
    return checkAgeAvailable();
}

// Reset cache (for testing)
export function resetAgeAvailability() {
    ageAvailable = null;
}

export function cypher(query: string, returnType = "v agtype") {
    return Effect.tryPromise({
        try: async () => {
            if (!(await checkAgeAvailable())) return [];
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
            if (!(await checkAgeAvailable())) return;
            await db.execute(
                sql.raw(`SELECT * FROM cypher('knowledge', $$ ${query} $$) AS (v agtype)`)
            );
        },
        catch: cause => new DatabaseError({ cause })
    });
}
