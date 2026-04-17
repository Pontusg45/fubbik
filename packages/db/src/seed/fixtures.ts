/**
 * Named-reference fixture DSL.
 *
 * Write fixtures like data ("chunk 'x' connects to chunk 'y' with relation Z"),
 * not UUIDs. The loader resolves `name` references to real IDs at insert time
 * and records the results in `ctx.ids.chunks[name]` so later modules can
 * reference the same chunks by name.
 *
 *   const fx: ChunkFixture[] = [
 *     { name: "intro", title: "Introduction", type: "document" },
 *     { name: "details", title: "Details", type: "reference" }
 *   ];
 *   await loadChunkFixtures(ctx, fx, codebaseId);
 *
 *   const links: ConnectionFixture[] = [
 *     { from: "intro", to: "details", relation: "references" }
 *   ];
 *   await loadConnectionFixtures(ctx, links);
 */

import { chunk, chunkConnection } from "../schema/chunk";
import { chunkCodebase } from "../schema/codebase";
import { chunkTag, tag } from "../schema/tag";
import { eq, and } from "drizzle-orm";
import { makeChunk, makeConnection, uuid } from "./factories";
import type { SeedContext } from "./context";

export interface ChunkFixture {
    /** Stable name for cross-references within this seed run. */
    name: string;
    title: string;
    type?: string;
    content?: string;
    summary?: string | null;
    rationale?: string | null;
    consequences?: string | null;
    /** Tag names to associate with this chunk (must already be created). */
    tags?: string[];
}

export interface ConnectionFixture {
    /** Name or ID of source chunk. Names resolve via ctx.ids.chunks. */
    from: string;
    to: string;
    relation: string;
}

/**
 * Insert chunks from a fixture list, register their IDs under ctx.ids.chunks,
 * optionally link them to a codebase and apply tags.
 */
export async function loadChunkFixtures(
    ctx: SeedContext,
    fixtures: ChunkFixture[],
    opts?: { codebaseId?: string }
): Promise<string[]> {
    if (fixtures.length === 0) return [];

    const rows = fixtures.map(fx => {
        const row = makeChunk({
            userId: ctx.userId,
            title: fx.title,
            type: fx.type,
            content: fx.content,
            summary: fx.summary,
            rationale: fx.rationale,
            consequences: fx.consequences
        });
        ctx.ids.chunks[fx.name] = row.id!;
        return row;
    });

    await ctx.db.insert(chunk).values(rows);

    if (opts?.codebaseId) {
        await ctx.db
            .insert(chunkCodebase)
            .values(rows.map(r => ({ chunkId: r.id!, codebaseId: opts.codebaseId! })));
    }

    // Tag associations — one query per fixture that declares tags (usually small).
    for (const fx of fixtures) {
        if (!fx.tags || fx.tags.length === 0) continue;
        const chunkId = ctx.ids.chunks[fx.name]!;
        for (const tagName of fx.tags) {
            const tagId = ctx.ids.tags[tagName];
            if (!tagId) {
                // Tag must exist — fail loud. Typos here cause silent holes later.
                throw new Error(`ChunkFixture "${fx.name}" references unknown tag "${tagName}"`);
            }
            await ctx.db.insert(chunkTag).values({ chunkId, tagId });
        }
    }

    ctx.counters["chunks"] = (ctx.counters["chunks"] ?? 0) + rows.length;
    return rows.map(r => r.id!);
}

/**
 * Insert connections by name. Resolves `from`/`to` via ctx.ids.chunks first,
 * then falls back to the raw string (so callers can pass IDs directly if they
 * prefer). Throws if a name doesn't resolve.
 */
export async function loadConnectionFixtures(
    ctx: SeedContext,
    fixtures: ConnectionFixture[]
): Promise<void> {
    if (fixtures.length === 0) return;

    const rows = fixtures.map(fx => {
        const from = ctx.ids.chunks[fx.from] ?? fx.from;
        const to = ctx.ids.chunks[fx.to] ?? fx.to;
        if (!from || !to) {
            throw new Error(`ConnectionFixture unresolved: ${fx.from} -> ${fx.to}`);
        }
        return makeConnection(from, to, fx.relation);
    });

    await ctx.db.insert(chunkConnection).values(rows);
    ctx.counters["connections"] = (ctx.counters["connections"] ?? 0) + rows.length;
}

/**
 * Resolve an existing tag by name from the DB into ctx.ids.tags. Useful when
 * the caller wants to reference tags seeded by another module without passing
 * IDs through.
 */
export async function registerTagsByName(ctx: SeedContext, names: string[]): Promise<void> {
    if (names.length === 0) return;
    const rows = await ctx.db.select().from(tag).where(eq(tag.userId, ctx.userId));
    for (const row of rows) {
        if (names.includes(row.name)) ctx.ids.tags[row.name] = row.id;
    }
    for (const name of names) {
        if (!ctx.ids.tags[name]) {
            throw new Error(`Tag "${name}" not found in DB — did you run the tags module first?`);
        }
    }
}

void and; // imported for callers that extend with composite where clauses
