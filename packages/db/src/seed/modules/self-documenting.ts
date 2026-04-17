/**
 * Self-documenting module.
 *
 * After every seed run, this module inserts a small set of chunks that describe
 * the seed system itself — architecture, how scenarios work, how to add a
 * module, how the fixture DSL is structured. Each chunk is linked via
 * applies_to globs to the relevant files under `packages/db/src/seed/`, so the
 * `context-for` endpoint surfaces the right explainer when an LLM touches that
 * directory.
 *
 * This is what the user meant by "self-documenting fubbik": the seed doesn't
 * just populate business data — it populates meta-data about its own design,
 * tagged `seed-system` + `self-documenting`, findable through every normal
 * discovery path (graph, search, /context, CLAUDE.md export).
 */

import { chunkAppliesTo } from "../../schema/applies-to";
import { loadChunkFixtures, loadConnectionFixtures, type ChunkFixture, type ConnectionFixture } from "../fixtures";
import { uuid } from "../factories";
import type { SeedContext } from "../context";

const META_CHUNKS: ChunkFixture[] = [
    {
        name: "seed-architecture",
        title: "Seed system: architecture overview",
        type: "document",
        summary: "Modular orchestrator + per-domain modules + factories + fixture DSL + post-seed verification.",
        content: `The fubbik seed is broken into domain modules under \`packages/db/src/seed/modules\`. Each module exports a \`seed(ctx)\` function that inserts rows for its domain (tags, chunks, plans, …) and optionally a \`reset(ctx)\` for per-module cleanup.

Modules communicate through a shared \`SeedContext\` with an id-registry (\`ctx.ids.chunks[name] = id\`) so later modules can reference earlier rows by semantic name instead of hardcoded UUIDs.

## Pieces
- \`seed/context.ts\` — SeedContext type, \`createContext\`, \`trySeed\` (strict error wrapper)
- \`seed/factories.ts\` — \`makeChunk\` / \`makeConnection\` / \`makeTag\` / \`makeCodebase\`
- \`seed/fixtures.ts\` — ChunkFixture + ConnectionFixture DSLs, \`loadChunkFixtures\`, \`loadConnectionFixtures\`
- \`seed/verify.ts\` — post-seed row counts + FK integrity probes
- \`seed/modules/*.ts\` — one file per domain
- \`seed/index.ts\` — orchestrator (CLI flags, module registry, scenarios)

## Run
- \`pnpm seed\` → scenario=demo
- \`pnpm seed --scenario=minimal\` → catalogs + self-doc only
- \`pnpm seed --only=chunks,connections\` → just those modules
- \`pnpm seed --skip=documents\` → skip a module

## Strict errors
Every step runs through \`trySeed()\`, which logs start/finish AND rethrows on failure. No more silent \`.catch(console.error)\`.`,
        rationale: "Adding a new kind of data used to mean editing a 2789-line monolith. Now it's a single ~80-line file in modules/.",
        tags: ["seed-system", "self-documenting", "architecture", "onboarding"]
    },
    {
        name: "seed-fixtures-dsl",
        title: "Seed system: fixture DSL",
        type: "reference",
        content: `Chunks and connections are declared as **data**, not imperative insert calls:

\`\`\`ts
const fixtures: ChunkFixture[] = [
    { name: "intro",   title: "Introduction", type: "document", tags: ["onboarding"] },
    { name: "details", title: "Details",      type: "reference" }
];
await loadChunkFixtures(ctx, fixtures, { codebaseId });

const links: ConnectionFixture[] = [
    { from: "intro", to: "details", relation: "references" }
];
await loadConnectionFixtures(ctx, links);
\`\`\`

The loader fills defaults (timestamps, flags, IDs), registers \`ctx.ids.chunks[name]\` for cross-refs, wires the chunk to a codebase, and applies tags — all in one call. Unknown tag names fail loud instead of silently dropping the association.`,
        tags: ["seed-system", "self-documenting", "reference"]
    },
    {
        name: "seed-scenarios",
        title: "Seed system: scenarios",
        type: "reference",
        content: `A scenario is a named subset of modules. Three are registered today:

- **minimal** — \`core\` + \`self-documenting\`. Enough to render the graph types/relations and describe the system. Use for CI or a fresh database where you don't need fixture data.
- **demo** — everything; the default. Produces the current \`pnpm seed\` experience.
- **extended** — hook for richer fixtures (add modules to the \`extended\` scenario list only; they won't run under \`demo\`).

Declared on each module entry in \`MODULE_REGISTRY\`:

\`\`\`ts
{ name: "tags", deps: ["core"], scenarios: ["demo", "extended"], ...tagsModule }
\`\`\``,
        tags: ["seed-system", "self-documenting", "reference"]
    },
    {
        name: "seed-adding-a-module",
        title: "Seed system: how to add a new module",
        type: "guide",
        content: `1. Create \`packages/db/src/seed/modules/<name>.ts\` with:
   \`\`\`ts
   export async function seed(ctx: SeedContext): Promise<void> { /* inserts */ }
   export async function reset(ctx: SeedContext): Promise<void> { /* cleanup */ } // optional
   \`\`\`
2. Import + register in \`seed/index.ts\` under \`MODULE_REGISTRY\`:
   \`\`\`ts
   import * as myModule from "./modules/my-module";
   // ...
   { name: "my-module", deps: ["core"], scenarios: ["demo"], ...myModule }
   \`\`\`
3. Add the appropriate probe to \`seed/verify.ts\` if you care about row-count drift.
4. If your data cross-references other modules, either populate \`ctx.ids.<domain>[name]\` in yours OR read from the registry in theirs.

The orchestrator runs modules in registry order; dependencies are documented via the \`deps\` array but not enforced by topo-sort yet — keep the registry array in dependency order.`,
        tags: ["seed-system", "self-documenting", "onboarding"]
    },
    {
        name: "seed-factories",
        title: "Seed system: factory helpers",
        type: "reference",
        content: `Instead of spelling out every chunk column on every insert, use factories:

\`\`\`ts
const row = makeChunk({ userId, title: "Foo", type: "convention" });
// → full NewChunk with sensible defaults for content, scope, origin, reviewStatus, etc.

const link = makeConnection(srcId, tgtId, "depends_on");
const tag = makeTag({ name: "frontend", tagTypeId, userId });
\`\`\`

Overrides win: pass \`summary\`, \`content\`, \`rationale\`, etc. as needed. The factory fills everything else.

Factories return schema-typed rows so \`ctx.db.insert(chunk).values([...factories])\` stays fully type-checked — no \`as NewChunk\` casts.`,
        tags: ["seed-system", "self-documenting", "reference"]
    },
    {
        name: "seed-verification",
        title: "Seed system: post-seed verification",
        type: "reference",
        content: `\`seed/verify.ts\` runs two sets of probes after all modules:

1. **Row counts** — one per table, printed as a table for visual diff between runs.
2. **FK integrity** — orphan chunk_tag rows, connections pointing at missing chunks, tasks without parents. Each should be zero; non-zero prints a warning that points at the reset-order bug.

Integrity failures don't block the seed (the data is already committed), but they print a loud warning. The usual fix is to reorder \`MODULE_REGISTRY\` so that children reset before their parents.`,
        tags: ["seed-system", "self-documenting", "reference"]
    }
];

const META_LINKS: ConnectionFixture[] = [
    { from: "seed-architecture", to: "seed-fixtures-dsl", relation: "part_of" },
    { from: "seed-architecture", to: "seed-scenarios", relation: "part_of" },
    { from: "seed-architecture", to: "seed-adding-a-module", relation: "part_of" },
    { from: "seed-architecture", to: "seed-factories", relation: "part_of" },
    { from: "seed-architecture", to: "seed-verification", relation: "part_of" },
    { from: "seed-fixtures-dsl", to: "seed-factories", relation: "depends_on" },
    { from: "seed-adding-a-module", to: "seed-architecture", relation: "references" },
    { from: "seed-architecture", to: "seed-system", relation: "references" }
];

// File-ref globs so the context-for-file endpoint surfaces the right explainer
// when an LLM touches the seed directory.
const META_APPLIES: Array<{ chunkName: string; pattern: string; note?: string }> = [
    { chunkName: "seed-architecture", pattern: "packages/db/src/seed/**", note: "Entry point + module registry" },
    { chunkName: "seed-fixtures-dsl", pattern: "packages/db/src/seed/fixtures.ts" },
    { chunkName: "seed-factories", pattern: "packages/db/src/seed/factories.ts" },
    { chunkName: "seed-scenarios", pattern: "packages/db/src/seed/index.ts" },
    { chunkName: "seed-adding-a-module", pattern: "packages/db/src/seed/modules/**" },
    { chunkName: "seed-verification", pattern: "packages/db/src/seed/verify.ts" }
];

export async function seed(ctx: SeedContext): Promise<void> {
    const codebaseId = ctx.ids.codebases["fubbik"];

    // If chunks module hasn't run (minimal scenario), the "seed-system" tag
    // reference in META_LINKS below won't resolve. Soft-handle by filtering.
    const haveSeedSystemChunk = !!ctx.ids.chunks["seed-system"];

    // Strip tag references to tags the tags module didn't register. Minimal
    // scenario skips the tags module entirely — we keep the meta-chunks
    // themselves but drop the tag links silently.
    const fixturesSafe: ChunkFixture[] = META_CHUNKS.map(fx => ({
        ...fx,
        tags: (fx.tags ?? []).filter(t => !!ctx.ids.tags[t])
    }));
    await loadChunkFixtures(ctx, fixturesSafe, codebaseId ? { codebaseId } : undefined);

    const linksToUse = haveSeedSystemChunk ? META_LINKS : META_LINKS.filter(l => l.to !== "seed-system" && l.from !== "seed-system");
    await loadConnectionFixtures(ctx, linksToUse);

    const appliesRows = META_APPLIES
        .map(a => ({
            id: uuid(),
            chunkId: ctx.ids.chunks[a.chunkName],
            pattern: a.pattern,
            note: a.note ?? null
        }))
        .filter(r => !!r.chunkId) as Array<{ id: string; chunkId: string; pattern: string; note: string | null }>;
    if (appliesRows.length > 0) {
        await ctx.db.insert(chunkAppliesTo).values(appliesRows);
        ctx.counters["self_doc_applies_to"] = appliesRows.length;
    }

    ctx.counters["self_doc_chunks"] = META_CHUNKS.length;
}

// No reset — chunks/connections cascade from the chunks module's reset.
