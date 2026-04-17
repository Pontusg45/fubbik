/**
 * Post-seed verification: sanity-check the DB against the counters modules
 * recorded, plus a couple of FK-integrity probes. Purely observational — we
 * don't block the seed on failure, just print a clear summary so drift is
 * visible immediately.
 */

import { sql } from "drizzle-orm";
import type { SeedContext } from "./context";

interface Probe {
    label: string;
    query: string;
}

const PROBES: Probe[] = [
    { label: "chunks", query: "SELECT count(*)::int FROM chunk" },
    { label: "chunk_tag links", query: "SELECT count(*)::int FROM chunk_tag" },
    { label: "chunk_codebase links", query: "SELECT count(*)::int FROM chunk_codebase" },
    { label: "chunk_connections", query: "SELECT count(*)::int FROM chunk_connection" },
    { label: "tag_types", query: "SELECT count(*)::int FROM tag_type" },
    { label: "tags", query: "SELECT count(*)::int FROM tag" },
    { label: "codebases", query: "SELECT count(*)::int FROM codebase" },
    { label: "documents", query: "SELECT count(*)::int FROM document" },
    { label: "requirements", query: "SELECT count(*)::int FROM requirement" },
    { label: "use_cases", query: "SELECT count(*)::int FROM use_case" },
    { label: "plans", query: "SELECT count(*)::int FROM plan" },
    { label: "plan_tasks", query: "SELECT count(*)::int FROM plan_task" },
    { label: "workspaces", query: "SELECT count(*)::int FROM workspace" },
    { label: "vocabulary entries", query: "SELECT count(*)::int FROM vocabulary_entry" },
    { label: "chunk_type (catalog)", query: "SELECT count(*)::int FROM chunk_type" },
    { label: "connection_relation (catalog)", query: "SELECT count(*)::int FROM connection_relation" }
];

const INTEGRITY_PROBES: Probe[] = [
    {
        label: "orphan chunk_tag rows",
        query: "SELECT count(*)::int FROM chunk_tag ct WHERE NOT EXISTS (SELECT 1 FROM chunk c WHERE c.id = ct.chunk_id)"
    },
    {
        label: "connections pointing at missing chunks",
        query:
            "SELECT count(*)::int FROM chunk_connection cc WHERE NOT EXISTS (SELECT 1 FROM chunk c WHERE c.id = cc.source_id) OR NOT EXISTS (SELECT 1 FROM chunk c WHERE c.id = cc.target_id)"
    },
    {
        label: "tasks without parent plan",
        query:
            "SELECT count(*)::int FROM plan_task pt WHERE NOT EXISTS (SELECT 1 FROM plan p WHERE p.id = pt.plan_id)"
    }
];

export async function verifySeed(ctx: SeedContext): Promise<void> {
    ctx.log("\n=== verification ===");

    const width = Math.max(...PROBES.map(p => p.label.length));
    for (const probe of PROBES) {
        const { rows } = await ctx.db.execute(sql.raw(probe.query));
        const count = Number((rows[0] as { count: number } | undefined)?.count ?? 0);
        ctx.log(`  ${probe.label.padEnd(width)}  ${String(count).padStart(5)}`);
    }

    ctx.log("\n=== integrity probes (should all be 0) ===");
    let anyBad = false;
    for (const probe of INTEGRITY_PROBES) {
        const { rows } = await ctx.db.execute(sql.raw(probe.query));
        const count = Number((rows[0] as { count: number } | undefined)?.count ?? 0);
        const ok = count === 0;
        if (!ok) anyBad = true;
        ctx.log(`  ${ok ? "✓" : "✗"} ${probe.label.padEnd(48)} ${String(count).padStart(5)}`);
    }

    if (anyBad) {
        ctx.log("\n⚠️  Integrity probes found orphaned rows. This usually means a");
        ctx.log("    module dropped rows whose children in another table are still live.");
        ctx.log("    Fix the reset order in the module registry.");
    }
}
