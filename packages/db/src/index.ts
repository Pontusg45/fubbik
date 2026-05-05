import { env } from "@fubbik/env/server";
import { drizzle } from "drizzle-orm/node-postgres";
import { Pool, type PoolClient, type PoolConfig } from "pg";

import * as schema from "./schema/index.js";

/** pg-pool supports this hook; awaiting it is not reflected in `@types/pg` PoolConfig yet. */
type PoolConfigWithOnConnect = PoolConfig & {
    onConnect?: (client: PoolClient) => void | Promise<void>;
};

// Use Pool `onConnect` (pg-pool awaits it before releasing the client to waiters).
// `pool.on("connect")` fires in the same turn as checkout — overlapping async queries
// on the client trigger pg's concurrent-query deprecation and subtle bugs.
const poolConfig: PoolConfigWithOnConnect = {
    connectionString: env.DATABASE_URL,
    onConnect: async client => {
        try {
            const { rows } = await client.query<{ age_installed: boolean }>(
                "SELECT EXISTS (SELECT 1 FROM pg_extension WHERE extname = 'age') AS age_installed"
            );
            if (!rows[0]?.age_installed) return;
            await client.query("LOAD 'age'");
            await client.query(`SET search_path = ag_catalog, "$user", public`);
        } catch {
            // AGE unavailable or LOAD failed — use SQL fallbacks (see repository code).
        }
    }
};
const pool = new Pool(poolConfig);

export const db = drizzle(pool, { schema });

export { dbEffect } from "./effect";
