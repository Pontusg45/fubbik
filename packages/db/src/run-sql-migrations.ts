import { readFileSync } from "fs";
import { Client } from "pg";

const DATABASE_URL = process.env.DATABASE_URL;
if (!DATABASE_URL) throw new Error("DATABASE_URL not set");

// Custom SQL migrations that Drizzle can't manage (extensions, indexes, seeds)
const files = ["src/migrations/0001_extensions_and_seeds.sql"];

const client = new Client({ connectionString: DATABASE_URL });
await client.connect();

for (const file of files) {
    try {
        const sql = readFileSync(file, "utf8");
        await client.query(sql);
        console.log(`  Applied ${file}`);
    } catch (err) {
        console.warn(`  Warning applying ${file}:`, (err as Error).message);
    }
}

await client.end();
console.log("SQL migrations complete.");
