import { readFileSync } from "fs";
import { Client } from "pg";

const DATABASE_URL = process.env.DATABASE_URL;
if (!DATABASE_URL) throw new Error("DATABASE_URL not set");

const files = [
    "src/migrations/0002_add_trgm.sql",
    "src/migrations/0003_add_retrieval_fields.sql",
    "src/migrations/0004_seed_templates.sql"
];

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

// Ensure embedding column exists even without pgvector (as text fallback)
try {
    await client.query("ALTER TABLE chunk ADD COLUMN IF NOT EXISTS embedding text;");
    console.log("  Ensured embedding column exists");
} catch (err) {
    console.warn("  Warning adding embedding column:", (err as Error).message);
}

await client.end();
console.log("SQL migrations complete.");
