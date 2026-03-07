/**
 * Scan the project codebase and insert chunks directly into the local Postgres database.
 *
 * Usage: bun run packages/db/src/scan.ts [--clear]
 *   --clear  Delete all existing chunks for dev user before inserting
 */
import { resolve } from "path";

import { config } from "dotenv";
import { eq } from "drizzle-orm";
import { drizzle } from "drizzle-orm/node-postgres";

import { user } from "./schema/auth";
import { chunk, chunkConnection } from "./schema/chunk";

// Reuse the CLI scanner
import { scanProject } from "../../../apps/cli/src/lib/scanner";

config({ path: resolve(import.meta.dirname, "../../../apps/server/.env") });

const DATABASE_URL = process.env.DATABASE_URL;
if (!DATABASE_URL) throw new Error("DATABASE_URL not set");

const db = drizzle(DATABASE_URL);

const DEV_USER_ID = "dev-user";
const shouldClear = process.argv.includes("--clear");

// Ensure dev user exists
const [existing] = await db.select().from(user).where(eq(user.id, DEV_USER_ID));
if (!existing) {
    await db.insert(user).values({
        id: DEV_USER_ID,
        name: "Dev User",
        email: "dev@localhost",
        emailVerified: false
    });
    console.log("Created dev user");
}

if (shouldClear) {
    const deleted = await db.delete(chunk).where(eq(chunk.userId, DEV_USER_ID)).returning({ id: chunk.id });
    console.log(`Cleared ${deleted.length} existing chunks`);
}

// Scan the project root
const projectRoot = resolve(import.meta.dirname, "../../..");
console.log(`\nScanning ${projectRoot}...\n`);

const scannedChunks = scanProject({ dir: projectRoot });
console.log(`Found ${scannedChunks.length} chunks from project scan\n`);

let inserted = 0;
const folderData = new Map<string, { indexId: string | null; childIds: string[]; titles: string[] }>();
const titleToId = new Map<string, string>();
const splitChildren: { childId: string; parentTitle: string }[] = [];

for (const c of scannedChunks) {
    const id = `scan-${crypto.randomUUID().slice(0, 8)}`;
    try {
        await db.insert(chunk).values({
            id,
            title: c.title,
            content: c.content,
            type: c.type,
            tags: c.tags,
            userId: DEV_USER_ID
        });
        inserted++;
        console.log(`  + [${c.type}] ${c.title}${c.parentTitle ? ` (split from "${c.parentTitle}")` : ""}`);

        titleToId.set(c.title, id);

        if (c.parentTitle) {
            splitChildren.push({ childId: id, parentTitle: c.parentTitle });
        }

        if (!folderData.has(c.folder)) folderData.set(c.folder, { indexId: null, childIds: [], titles: [] });
        const folder = folderData.get(c.folder)!;
        if (c.isIndex) {
            folder.indexId = id;
        } else if (!c.parentTitle) {
            folder.childIds.push(id);
            folder.titles.push(c.title);
        }
    } catch (e) {
        console.log(`  x [${c.type}] ${c.title}: ${(e as Error).message}`);
    }
}

// Create index chunks for folders that don't have one, then link children to index
let connections = 0;
for (const [folder, data] of folderData) {
    if (data.childIds.length === 0) continue;

    let indexId = data.indexId;

    // Create an index chunk if the folder doesn't have a README/index
    if (!indexId) {
        indexId = `scan-${crypto.randomUUID().slice(0, 8)}`;
        const folderName = folder === "." ? "Project" : folder.split("/").pop()!;
        const indexContent = data.titles.map(t => `- ${t}`).join("\n");
        await db.insert(chunk).values({
            id: indexId,
            title: `${folderName} Index`,
            content: `Documents in \`${folder}\`:\n\n${indexContent}`,
            type: "reference",
            tags: ["index", ...folder.split("/").filter(Boolean)],
            userId: DEV_USER_ID
        });
        inserted++;
        console.log(`  + [reference] ${folderName} Index (auto-generated)`);
    }

    // Connect each child to the index
    for (const childId of data.childIds) {
        try {
            await db.insert(chunkConnection).values({
                id: `conn-${crypto.randomUUID().slice(0, 8)}`,
                sourceId: childId,
                targetId: indexId,
                relation: "part_of"
            });
            connections++;
        } catch {
            // skip duplicates
        }
    }
    console.log(`  ~ ${folder}: ${data.childIds.length} chunks → ${data.indexId ? "existing" : "generated"} index`);
}

// Create connections for auto-split chunks to their parent
for (const { childId, parentTitle } of splitChildren) {
    const parentId = titleToId.get(parentTitle);
    if (!parentId) continue;
    try {
        await db.insert(chunkConnection).values({
            id: `conn-${crypto.randomUUID().slice(0, 8)}`,
            sourceId: childId,
            targetId: parentId,
            relation: "part_of"
        });
        connections++;
    } catch {
        // skip duplicates
    }
}

console.log(`\nInserted ${inserted} chunks, ${connections} connections`);
process.exit(0);
