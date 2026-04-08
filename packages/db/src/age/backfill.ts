// packages/db/src/age/backfill.ts
import { resolve } from "path";

import { config } from "dotenv";
import { sql } from "drizzle-orm";
import { drizzle } from "drizzle-orm/node-postgres";

import { chunk, chunkConnection } from "../schema/chunk";
import { requirement, requirementChunk } from "../schema/requirement";
import { requirementDependency } from "../schema/requirement-dependency";

config({ path: resolve(import.meta.dirname, "../../../../apps/server/.env") });

const DATABASE_URL = process.env.DATABASE_URL;
if (!DATABASE_URL) throw new Error("DATABASE_URL not set");

const db = drizzle(DATABASE_URL);

await db.execute(sql`LOAD 'age'`);
await db.execute(sql.raw(`SET search_path = ag_catalog, "$user", public`));

function cypherExec(query: string) {
    return db.execute(sql.raw(`SELECT * FROM cypher('knowledge', $$ ${query} $$) AS (v agtype)`));
}

console.log("\nBackfilling AGE graph...");

// 1. Chunk vertices
const allChunks = await db.select({ id: chunk.id }).from(chunk);
for (const c of allChunks) {
    await cypherExec(`MERGE (:chunk {id: '${c.id}'})`);
}
console.log(`  ✓ ${allChunks.length} chunk vertices`);

// 2. Requirement vertices
const allRequirements = await db.select({ id: requirement.id }).from(requirement);
for (const r of allRequirements) {
    await cypherExec(`MERGE (:requirement {id: '${r.id}'})`);
}
console.log(`  ✓ ${allRequirements.length} requirement vertices`);

// 3. connects edges
const allConnections = await db
    .select({
        id: chunkConnection.id,
        sourceId: chunkConnection.sourceId,
        targetId: chunkConnection.targetId,
        relation: chunkConnection.relation
    })
    .from(chunkConnection);
for (const conn of allConnections) {
    await cypherExec(
        `MATCH (a:chunk {id: '${conn.sourceId}'}), (b:chunk {id: '${conn.targetId}'})
         MERGE (a)-[:connects {id: '${conn.id}', relation: '${conn.relation}'}]->(b)`
    );
}
console.log(`  ✓ ${allConnections.length} connects edges`);

// 4. depends_on edges
const allDeps = await db
    .select({
        requirementId: requirementDependency.requirementId,
        dependsOnId: requirementDependency.dependsOnId
    })
    .from(requirementDependency);
for (const dep of allDeps) {
    await cypherExec(
        `MATCH (a:requirement {id: '${dep.requirementId}'}), (b:requirement {id: '${dep.dependsOnId}'})
         MERGE (a)-[:depends_on]->(b)`
    );
}
console.log(`  ✓ ${allDeps.length} depends_on edges`);

// 5. covers edges
const allCovers = await db
    .select({
        requirementId: requirementChunk.requirementId,
        chunkId: requirementChunk.chunkId
    })
    .from(requirementChunk);
for (const rc of allCovers) {
    await cypherExec(
        `MATCH (r:requirement {id: '${rc.requirementId}'}), (c:chunk {id: '${rc.chunkId}'})
         MERGE (r)-[:covers]->(c)`
    );
}
console.log(`  ✓ ${allCovers.length} covers edges`);

console.log("\n✅ Backfill complete");
process.exit(0);
