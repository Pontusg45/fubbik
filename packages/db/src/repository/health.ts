import { sql } from "drizzle-orm";

import { db, dbEffect } from "../index";

export function checkDbConnectivity() {
    return dbEffect(async () => {
            await db.execute(sql`SELECT 1`);
            return true;
        });
}
