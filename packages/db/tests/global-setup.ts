import { randomUUID } from "node:crypto";
import { readdir, readFile } from "node:fs/promises";

import { Client } from "pg";
import type { TestProject } from "vitest/node";

export default async function setup(project: TestProject) {
    const adminUrl = process.env.FUBBIK_TEST_DATABASE_URL ?? "postgres://postgres:password@localhost:5434/postgres";
    const admin = new Client({ connectionString: adminUrl });
    await admin.connect().catch(error => {
        throw new Error("Start the test database with ./scripts/rust-test-db.sh start, or set FUBBIK_TEST_DATABASE_URL.", { cause: error });
    });
    // Only this generated database is migrated or dropped. DATABASE_URL is intentionally ignored.
    const name = `fubbik_vitest_${randomUUID().replaceAll("-", "")}`;
    const databaseUrl = new URL(adminUrl);
    databaseUrl.pathname = `/${name}`;
    let created = false;
    const cleanup = async () => {
        try {
            if (created) await admin.query(`DROP DATABASE "${name}" WITH (FORCE)`);
        } finally {
            await admin.end();
        }
    };
    try {
        await admin.query(`CREATE DATABASE "${name}"`);
        created = true;
        const migrationClient = new Client({ connectionString: databaseUrl.href });
        try {
            await migrationClient.connect();
            const directory = new URL("../../../crates/fubbik-db/migrations/", import.meta.url);
            const migrations = (await readdir(directory)).filter(file => file.endsWith(".sql")).sort();
            // Each migration depends on the preceding schema changes.
            for (const file of migrations) {
                // eslint-disable-next-line no-await-in-loop
                await migrationClient.query(await readFile(new URL(file, directory), "utf8"));
            }
        } finally {
            await migrationClient.end();
        }
        project.provide("databaseUrl", databaseUrl.href);
        return cleanup;
    } catch (error) {
        await cleanup();
        throw error;
    }
}
