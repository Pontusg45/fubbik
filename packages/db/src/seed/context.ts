/**
 * Runtime context threaded through every seed module.
 *
 * Modules communicate through the `ids` registry instead of hardcoding UUIDs
 * across files. Module N records what it created under `ctx.ids.<domain>.<name>`;
 * module N+1 reads from that map. Example:
 *
 *   // In tags module:
 *   ctx.ids.tagTypes.feature = newUuid;
 *
 *   // In chunks module later:
 *   const ttId = ctx.ids.tagTypes.feature;
 *
 * This keeps the cross-references explicit, auto-completes in editors, and
 * fails loudly if a dependency hasn't run.
 */

import type { db as dbInstance } from "../index";

export type Database = typeof dbInstance;

export interface SeedContext {
    db: Database;
    userId: string;
    scenario: ScenarioName;
    /** Registry of named IDs so modules can reference each other's rows. */
    ids: {
        codebases: Record<string, string>;
        tagTypes: Record<string, string>;
        tags: Record<string, string>;
        chunks: Record<string, string>;
        documents: Record<string, string>;
        requirements: Record<string, string>;
        useCases: Record<string, string>;
        plans: Record<string, string>;
        planTasks: Record<string, string>;
        workspaces: Record<string, string>;
    };
    /** Per-module counters the verifier can compare against expected shapes. */
    counters: Record<string, number>;
    log: (msg: string) => void;
}

export type ScenarioName = "minimal" | "demo" | "extended";

/**
 * Build a fresh SeedContext. The id-registry starts empty; modules populate it.
 */
export function createContext(params: {
    db: Database;
    userId: string;
    scenario: ScenarioName;
    quiet?: boolean;
}): SeedContext {
    return {
        db: params.db,
        userId: params.userId,
        scenario: params.scenario,
        ids: {
            codebases: {},
            tagTypes: {},
            tags: {},
            chunks: {},
            documents: {},
            requirements: {},
            useCases: {},
            plans: {},
            planTasks: {},
            workspaces: {}
        },
        counters: {},
        log: params.quiet ? () => {} : (msg: string) => console.log(msg)
    };
}

/**
 * Strict wrapper around a seeding step. Logs start/finish + timing, and
 * RE-THROWS any error instead of silently logging. A broken fixture should fail
 * the seed, not mask itself — previously `.catch(e => console.error(...))` was
 * masking real schema/data issues for years.
 */
export async function trySeed(label: string, fn: () => Promise<void>, ctx: SeedContext): Promise<void> {
    const start = Date.now();
    try {
        await fn();
        const dur = Date.now() - start;
        ctx.log(`  ✓ ${label} (${dur}ms)`);
    } catch (err) {
        ctx.log(`  ✗ ${label}`);
        throw err;
    }
}
