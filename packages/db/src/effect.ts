import { Effect } from "effect";

import { DatabaseError } from "./errors";

/**
 * Wrap an async operation (typically a Drizzle query) so failures surface as
 * a tagged `DatabaseError` instead of a raw throw. Collapses the 250+ inline
 * `Effect.tryPromise({ try, catch })` blocks repeated across every repo file.
 *
 * Repo code reads:
 *   return dbEffect(async () => {
 *       const [row] = await db.select().from(table).where(...);
 *       return row ?? null;
 *   });
 *
 * The function name stays short because it's a repo-file workhorse.
 */
export function dbEffect<T>(fn: () => Promise<T>): Effect.Effect<T, DatabaseError> {
    return Effect.tryPromise({
        try: fn,
        catch: cause => new DatabaseError({ cause }),
    });
}
