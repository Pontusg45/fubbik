/**
 * Post-seed verification: sanity-check row counts and fail when an integrity
 * probe finds orphaned or mismatched rows. A successful seed is a verified
 * seed; callers never need to interpret warning-only output.
 */

import { sql } from "drizzle-orm";

import type { SeedContext } from "./context";

interface Probe {
    label: string;
    query: string;
}

export interface SeedIntegrityResult {
    label: string;
    count: number;
}

export function assertSeedIntegrity(results: readonly SeedIntegrityResult[]): void {
    const failures = results.filter(result => result.count !== 0);
    if (failures.length === 0) return;
    throw new Error(`Seed integrity failed: ${failures.map(result => `${result.label}: ${result.count}`).join(", ")}`);
}

const PROBES: Probe[] = [
    { label: "chunks", query: "SELECT count(*)::int FROM chunk" },
    { label: "chunk_tag links", query: "SELECT count(*)::int FROM chunk_tag" },
    { label: "chunk_space links", query: "SELECT count(*)::int FROM chunk_space" },
    { label: "chunk_connections", query: "SELECT count(*)::int FROM chunk_connection" },
    { label: "tag_types", query: "SELECT count(*)::int FROM tag_type" },
    { label: "tags", query: "SELECT count(*)::int FROM tag" },
    { label: "spaces", query: "SELECT count(*)::int FROM space" },
    { label: "documents", query: "SELECT count(*)::int FROM document" },
    { label: "requirements", query: "SELECT count(*)::int FROM requirement" },
    { label: "use_cases", query: "SELECT count(*)::int FROM use_case" },
    { label: "plans", query: "SELECT count(*)::int FROM plan" },
    { label: "plan_tasks", query: "SELECT count(*)::int FROM plan_task" },
    { label: "agent_runs", query: "SELECT count(*)::int FROM agent_run" },
    { label: "task_claims", query: "SELECT count(*)::int FROM plan_task_claim" },
    { label: "coordination entries", query: "SELECT count(*)::int FROM coordination_entry" },
    { label: "workspaces", query: "SELECT count(*)::int FROM workspace" },
    { label: "vocabulary entries", query: "SELECT count(*)::int FROM vocabulary_entry" },
    { label: "chunk_type (catalog)", query: "SELECT count(*)::int FROM chunk_type" },
    { label: "connection_relation (catalog)", query: "SELECT count(*)::int FROM connection_relation" },
    { label: "behavior_matrices", query: "SELECT count(*)::int FROM behavior_matrix" },
    { label: "behavior_cells", query: "SELECT count(*)::int FROM behavior_cell" }
];

const INTEGRITY_PROBES: Probe[] = [
    {
        label: "orphan chunk_tag rows",
        query: "SELECT count(*)::int FROM chunk_tag ct WHERE NOT EXISTS (SELECT 1 FROM chunk c WHERE c.id = ct.chunk_id)"
    },
    {
        label: "connections pointing at missing chunks",
        query: "SELECT count(*)::int FROM chunk_connection cc WHERE NOT EXISTS (SELECT 1 FROM chunk c WHERE c.id = cc.source_id) OR NOT EXISTS (SELECT 1 FROM chunk c WHERE c.id = cc.target_id)"
    },
    {
        label: "tasks without parent plan",
        query: "SELECT count(*)::int FROM plan_task pt WHERE NOT EXISTS (SELECT 1 FROM plan p WHERE p.id = pt.plan_id)"
    },
    {
        label: "coordination claims with mismatched references",
        query: "SELECT count(*)::int FROM plan_task_claim c LEFT JOIN plan_task t ON t.id = c.task_id AND t.plan_id = c.plan_id LEFT JOIN agent_run r ON r.id = c.agent_run_id AND r.plan_id = c.plan_id WHERE t.id IS NULL OR r.id IS NULL"
    },
    {
        label: "coordination entries with mismatched references",
        query: "SELECT count(*)::int FROM coordination_entry e LEFT JOIN agent_run a ON a.id = e.author_run_id AND a.plan_id = e.plan_id LEFT JOIN agent_run r ON r.id = e.recipient_run_id AND r.plan_id = e.plan_id LEFT JOIN plan_task t ON t.id = e.task_id AND t.plan_id = e.plan_id LEFT JOIN coordination_entry p ON p.id = e.reply_to_id AND p.plan_id = e.plan_id WHERE a.id IS NULL OR (e.recipient_run_id IS NOT NULL AND r.id IS NULL) OR (e.task_id IS NOT NULL AND t.id IS NULL) OR (e.reply_to_id IS NOT NULL AND p.id IS NULL)"
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
    const integrityResults: SeedIntegrityResult[] = [];
    for (const probe of INTEGRITY_PROBES) {
        const { rows } = await ctx.db.execute(sql.raw(probe.query));
        const count = Number((rows[0] as { count: number } | undefined)?.count ?? 0);
        const ok = count === 0;
        integrityResults.push({ label: probe.label, count });
        ctx.log(`  ${ok ? "✓" : "✗"} ${probe.label.padEnd(48)} ${String(count).padStart(5)}`);
    }

    if (integrityResults.some(result => result.count !== 0)) {
        ctx.log("\n⚠️  Integrity probes found orphaned rows. This usually means a");
        ctx.log("    module dropped rows whose children in another table are still live.");
        ctx.log("    Fix the reset order in the module registry.");
    }
    assertSeedIntegrity(integrityResults);
}
