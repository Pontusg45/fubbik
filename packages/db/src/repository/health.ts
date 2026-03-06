import { sql } from "drizzle-orm";
import { Effect } from "effect";

import { DatabaseError } from "../errors";
import { db } from "../index";

export function checkDbConnectivity() {
    return Effect.tryPromise({
        try: async () => {
            await db.execute(sql`SELECT 1`);
            return true;
        },
        catch: cause => new DatabaseError({ cause })
    });
}
