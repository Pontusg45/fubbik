import { Effect } from "effect";
import { sql } from "drizzle-orm";
import { db } from "../index";
import { DatabaseError } from "../errors";

export function checkDbConnectivity() {
  return Effect.tryPromise({
    try: async () => {
      await db.execute(sql`SELECT 1`);
      return true;
    },
    catch: (cause) => new DatabaseError({ cause }),
  });
}
