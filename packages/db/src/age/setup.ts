// packages/db/src/age/setup.ts
import { resolve } from "path";

import { config } from "dotenv";
import { sql } from "drizzle-orm";
import { drizzle } from "drizzle-orm/node-postgres";

config({ path: resolve(import.meta.dirname, "../../../../apps/server/.env") });

const DATABASE_URL = process.env.DATABASE_URL;
if (!DATABASE_URL) throw new Error("DATABASE_URL not set");

const db = drizzle(DATABASE_URL);

console.log("Setting up Apache AGE...");

await db.execute(sql`CREATE EXTENSION IF NOT EXISTS age`);
console.log("  ✓ Extension created");

await db.execute(sql`LOAD 'age'`);
await db.execute(sql.raw(`SET search_path = ag_catalog, "$user", public`));
console.log("  ✓ AGE loaded");

try {
    await db.execute(sql.raw(`SELECT create_graph('knowledge')`));
    console.log("  ✓ Graph 'knowledge' created");
} catch (e: any) {
    if (e?.message?.includes("already exists")) {
        console.log("  ✓ Graph 'knowledge' already exists");
    } else {
        throw e;
    }
}

console.log("\n✅ AGE setup complete");

await import("./backfill.js");
