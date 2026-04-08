import { env } from "@fubbik/env/server";
import { drizzle } from "drizzle-orm/node-postgres";
import pg from "pg";

import * as schema from "./schema/index.js";

const pool = new pg.Pool({ connectionString: env.DATABASE_URL });

// Initialize AGE extension on each new connection
pool.on("connect", async (client) => {
    try {
        await client.query("LOAD 'age'");
        await client.query(`SET search_path = ag_catalog, "$user", public`);
    } catch {
        // AGE not installed — silently skip (fallback mode)
    }
});

export const db = drizzle(pool, { schema });
