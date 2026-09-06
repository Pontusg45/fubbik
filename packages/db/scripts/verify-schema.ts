import { SCHEMA_DRIFT_ALLOWLIST, verifySchema } from "../src/schema-parity";

const databaseUrl = process.env.DATABASE_URL;
if (!databaseUrl) throw new Error("DATABASE_URL is required");

await verifySchema(databaseUrl);
console.log("SQL migrations and Drizzle schema are in parity");
for (const allowance of SCHEMA_DRIFT_ALLOWLIST) {
    console.log(`allowed during cutover: ${allowance.issue} (${allowance.reason})`);
}
