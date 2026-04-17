/**
 * Orchestrator for fubbik's database seed.
 *
 * Usage:
 *   pnpm seed                              # default scenario: "demo"
 *   pnpm seed --scenario=minimal          # catalogs + self-doc only
 *   pnpm seed --scenario=extended         # demo + extra fixtures
 *   pnpm seed --only=core,tags            # run only these modules
 *   pnpm seed --skip=documents            # run everything except these
 *   pnpm seed --reset=none                # skip the user-scoped reset
 *   pnpm seed --quiet                     # suppress per-step logs
 *
 * Modules are declared in MODULE_REGISTRY (below). Each exports a `seed(ctx)`
 * function; the orchestrator threads a shared SeedContext through so modules
 * can reference each other's rows by name without hardcoded UUIDs.
 *
 * Architecture:
 *   - context.ts   — SeedContext type + `trySeed()` strict wrapper
 *   - factories.ts — makeChunk / makeConnection / makeTag / makeCodebase
 *   - fixtures.ts  — named-reference DSL for chunks + connections
 *   - verify.ts    — post-seed row counts + FK integrity probes
 *   - modules/     — one file per domain (core, tags, chunks, plans, …)
 *
 * Adding a new module:
 *   1. Create `modules/<name>.ts` exporting `seed(ctx: SeedContext): Promise<void>`
 *      and optionally a `reset(ctx)` for the per-module reset path.
 *   2. Register in MODULE_REGISTRY below with its dependency list.
 *   3. Add to the scenarios that should include it (default is "demo").
 */

import { resolve } from "path";
import { config } from "dotenv";
import { drizzle } from "drizzle-orm/node-postgres";
import { eq } from "drizzle-orm";

import { user } from "../schema/auth";
import { createContext, trySeed, type ScenarioName, type SeedContext } from "./context";
import { verifySeed } from "./verify";

// Modules — import each and register below.
import * as coreModule from "./modules/core";
import * as codebasesModule from "./modules/codebases";
import * as tagsModule from "./modules/tags";
import * as chunksModule from "./modules/chunks";
import * as connectionsModule from "./modules/connections";
import * as fileLinksModule from "./modules/file-links";
import * as useCasesModule from "./modules/use-cases";
import * as requirementsModule from "./modules/requirements";
import * as plansModule from "./modules/plans";
import * as documentsModule from "./modules/documents";
import * as vocabularyModule from "./modules/vocabulary";
import * as workspacesModule from "./modules/workspaces";
import * as collectionsModule from "./modules/collections";
import * as selfDocModule from "./modules/self-documenting";

config({ path: resolve(import.meta.dirname, "../../../../apps/server/.env") });

const DATABASE_URL = process.env.DATABASE_URL;
if (!DATABASE_URL) throw new Error("DATABASE_URL not set");

const db = drizzle(DATABASE_URL);

const DEV_USER_ID = "dev-user";

/**
 * Each module declares:
 *   - `name`: short slug for CLI selection
 *   - `deps`: names of modules that must run first
 *   - `scenarios`: which scenario names include this module
 *   - `seed(ctx)`: do the inserts
 *   - `reset(ctx)` (optional): per-module cleanup, called when --reset includes this module
 */
interface ModuleEntry {
    name: string;
    deps: string[];
    scenarios: ScenarioName[];
    seed: (ctx: SeedContext) => Promise<void>;
    reset?: (ctx: SeedContext) => Promise<void>;
}

const MODULE_REGISTRY: ModuleEntry[] = [
    { name: "core", deps: [], scenarios: ["minimal", "demo", "extended"], ...coreModule },
    { name: "codebases", deps: ["core"], scenarios: ["demo", "extended"], ...codebasesModule },
    { name: "tags", deps: ["core"], scenarios: ["demo", "extended"], ...tagsModule },
    { name: "chunks", deps: ["core", "codebases", "tags"], scenarios: ["demo", "extended"], ...chunksModule },
    { name: "connections", deps: ["chunks"], scenarios: ["demo", "extended"], ...connectionsModule },
    { name: "file-links", deps: ["chunks"], scenarios: ["demo", "extended"], ...fileLinksModule },
    { name: "use-cases", deps: ["core", "codebases"], scenarios: ["demo", "extended"], ...useCasesModule },
    { name: "requirements", deps: ["core", "codebases", "use-cases", "chunks"], scenarios: ["demo", "extended"], ...requirementsModule },
    { name: "plans", deps: ["core", "codebases", "requirements", "chunks"], scenarios: ["demo", "extended"], ...plansModule },
    { name: "documents", deps: ["core", "codebases"], scenarios: ["demo", "extended"], ...documentsModule },
    { name: "vocabulary", deps: ["core", "codebases"], scenarios: ["demo", "extended"], ...vocabularyModule },
    { name: "workspaces", deps: ["core", "codebases"], scenarios: ["demo", "extended"], ...workspacesModule },
    { name: "collections", deps: ["core", "chunks"], scenarios: ["demo", "extended"], ...collectionsModule },
    { name: "self-documenting", deps: ["core", "codebases", "tags"], scenarios: ["minimal", "demo", "extended"], ...selfDocModule }
];

// ---------------------------------------------------------------------------
// CLI parsing
// ---------------------------------------------------------------------------

interface CliFlags {
    scenario: ScenarioName;
    only?: Set<string>;
    skip?: Set<string>;
    resetPolicy: "auto" | "none";
    quiet: boolean;
}

function parseFlags(argv: string[]): CliFlags {
    const flags: CliFlags = {
        scenario: "demo",
        resetPolicy: "auto",
        quiet: false
    };
    for (const arg of argv) {
        if (arg.startsWith("--scenario=")) {
            const val = arg.slice("--scenario=".length);
            if (val !== "minimal" && val !== "demo" && val !== "extended") {
                throw new Error(`Unknown scenario: ${val}`);
            }
            flags.scenario = val;
        } else if (arg.startsWith("--only=")) {
            flags.only = new Set(arg.slice("--only=".length).split(","));
        } else if (arg.startsWith("--skip=")) {
            flags.skip = new Set(arg.slice("--skip=".length).split(","));
        } else if (arg === "--reset=none") {
            flags.resetPolicy = "none";
        } else if (arg === "--quiet") {
            flags.quiet = true;
        }
    }
    return flags;
}

function selectedModules(flags: CliFlags): ModuleEntry[] {
    let mods = MODULE_REGISTRY.filter(m => m.scenarios.includes(flags.scenario));
    if (flags.only) mods = mods.filter(m => flags.only!.has(m.name));
    if (flags.skip) mods = mods.filter(m => !flags.skip!.has(m.name));

    // Validate dep ordering: every dep must either be in the selected set or
    // already in the DB from a previous run. We don't topo-sort here because
    // MODULE_REGISTRY is already in dependency order.
    const selected = new Set(mods.map(m => m.name));
    for (const m of mods) {
        for (const dep of m.deps) {
            if (!selected.has(dep) && flags.resetPolicy !== "none") {
                // dep may or may not be in the DB — warn, not fail. The module
                // itself will fail loudly if its reads come back empty.
            }
        }
    }
    return mods;
}

// ---------------------------------------------------------------------------
// Reset
// ---------------------------------------------------------------------------

async function resetForUser(ctx: SeedContext, mods: ModuleEntry[]): Promise<void> {
    // Reset in reverse dependency order so children go before parents.
    for (const m of [...mods].reverse()) {
        if (!m.reset) continue;
        await trySeed(`reset:${m.name}`, () => m.reset!(ctx), ctx);
    }
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

async function ensureUser(userId: string) {
    const [existing] = await db.select().from(user).where(eq(user.id, userId));
    if (!existing) {
        await db.insert(user).values({
            id: userId,
            name: "Dev User",
            email: "dev@localhost",
            emailVerified: false
        });
    }
}

async function main() {
    const flags = parseFlags(process.argv.slice(2));
    const ctx = createContext({ db, userId: DEV_USER_ID, scenario: flags.scenario, quiet: flags.quiet });

    ctx.log(`\nfubbik seed — scenario=${flags.scenario}${flags.only ? `, only=${[...flags.only].join(",")}` : ""}${flags.skip ? `, skip=${[...flags.skip].join(",")}` : ""}`);
    ctx.log("─".repeat(60));

    await ensureUser(DEV_USER_ID);

    const mods = selectedModules(flags);
    ctx.log(`  → ${mods.length} module(s) selected`);

    if (flags.resetPolicy === "auto") {
        ctx.log("\n=== reset (reverse order) ===");
        await resetForUser(ctx, mods);
    }

    ctx.log("\n=== seed ===");
    for (const m of mods) {
        await trySeed(m.name, () => m.seed(ctx), ctx);
    }

    await verifySeed(ctx);
    ctx.log("\n✅ seed complete\n");
}

main().catch(err => {
    console.error("\n❌ seed FAILED:", err);
    process.exit(1);
});
