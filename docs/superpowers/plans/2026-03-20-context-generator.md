# Context File Generator Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `fubbik context --for <file>` CLI command that generates a focused context document with all chunks relevant to a specific file, suitable for piping into AI assistants.

**Architecture:** New CLI command that queries the server for chunks matching a file path via fileRef lookup + appliesTo glob matching + optional semantic search. Outputs structured markdown. Also adds a new API endpoint that aggregates these lookups server-side for efficiency.

**Tech Stack:** Commander.js, Bun, Elysia API

---

## File Structure

### New files:
- `packages/api/src/context/routes.ts` — API endpoint for context generation
- `packages/api/src/context/service.ts` — Service aggregating file-ref + appliesTo + semantic lookups
- `apps/cli/src/commands/context-for.ts` — CLI command

### Files to modify:
- `packages/api/src/index.ts` — Mount context routes
- `apps/cli/src/index.ts` — Register command

---

## Task 1: Context API Endpoint

**Files:**
- Create: `packages/api/src/context/service.ts`
- Create: `packages/api/src/context/routes.ts`
- Modify: `packages/api/src/index.ts`

- [ ] **Step 1: Create context service**

```ts
// packages/api/src/context/service.ts
import { Effect } from "effect";
// NOTE: lookupChunksByFilePath returns { chunkId, chunkTitle, chunkType, refId, path, anchor, relation }
// It does NOT return content or summary — need a separate chunk fetch for those.
// Also: the export name is `listChunks` (not `listChunksRepo`).
import { lookupChunksByFilePath, getAppliesToForChunk, listChunks, getChunkById } from "@fubbik/db/repository";

interface ContextChunk {
    id: string;
    title: string;
    type: string;
    content: string;
    summary: string | null;
    matchReason: string; // "file-ref" | "applies-to" | "semantic"
}

export function getContextForFile(params: {
    path: string;
    userId: string;
    codebaseId?: string;
    format?: "markdown" | "json";
}) {
    return Effect.gen(function* () {
        const chunks: ContextChunk[] = [];
        const seenIds = new Set<string>();

        // 1. Direct file-ref matches
        // NOTE: lookupChunksByFilePath returns { chunkId, chunkTitle, chunkType, ... }
        // It does NOT include content/summary — fetch those separately.
        const fileRefMatches = yield* lookupChunksByFilePath(params.path, params.userId);
        for (const match of fileRefMatches) {
            if (!seenIds.has(match.chunkId)) {
                seenIds.add(match.chunkId);
                // Fetch full chunk to get content + summary
                const fullChunk = yield* getChunkById(match.chunkId);
                chunks.push({
                    id: match.chunkId,
                    title: match.chunkTitle,
                    type: match.chunkType,
                    content: fullChunk.content,
                    summary: fullChunk.summary,
                    matchReason: "file-ref",
                });
            }
        }

        // 2. AppliesTo glob matches — fetch all user chunks with appliesTo patterns
        // and match against the file path
        // NOTE: This is a simplified approach. For large knowledge bases,
        // consider a server-side glob index.
        const allChunks = yield* listChunks({
            userId: params.userId,
            codebaseId: params.codebaseId,
            limit: 200,
            offset: 0,
        });

        for (const c of allChunks.chunks) {
            if (seenIds.has(c.id)) continue;
            const appliesTo = yield* getAppliesToForChunk(c.id);
            for (const at of appliesTo) {
                if (simpleGlobMatch(at.pattern, params.path)) {
                    seenIds.add(c.id);
                    chunks.push({
                        id: c.id,
                        title: c.title,
                        type: c.type,
                        content: c.content,
                        summary: c.summary,
                        matchReason: "applies-to",
                    });
                    break;
                }
            }
        }

        return chunks;
    });
}

function simpleGlobMatch(pattern: string, path: string): boolean {
    const regexStr = pattern
        .replace(/\./g, "\\.")
        .replace(/\*\*/g, "{{GLOBSTAR}}")
        .replace(/\*/g, "[^/]*")
        .replace(/\?/g, "[^/]")
        .replace(/\{\{GLOBSTAR\}\}/g, ".*");
    return new RegExp(`^${regexStr}$`).test(path);
}
```

**Note:** Read the actual repo function signatures first — `lookupChunksByFilePath` and `listChunksRepo` return Effect types. The above uses `Effect.gen` which should match codebase patterns. Verify the return shapes.

- [ ] **Step 2: Create context routes**

```ts
// packages/api/src/context/routes.ts
import { Elysia, t } from "elysia";
import { Effect } from "effect";
import { requireSession } from "../auth/session";
import { getContextForFile } from "./service";

export const contextForFileRoutes = new Elysia()
    .get(
        "/context/for-file",
        ctx => Effect.runPromise(
            requireSession(ctx).pipe(
                Effect.flatMap(session =>
                    getContextForFile({
                        path: ctx.query.path,
                        userId: session.user.id,
                        codebaseId: ctx.query.codebaseId,
                        format: (ctx.query.format as "markdown" | "json") || "markdown",
                    })
                )
            )
        ),
        {
            query: t.Object({
                path: t.String(),
                codebaseId: t.Optional(t.String()),
                format: t.Optional(t.String()),
            }),
        }
    );
```

- [ ] **Step 3: Mount routes**

In `packages/api/src/index.ts`, import and `.use()` the context routes.

- [ ] **Step 4: Commit**

```bash
git add packages/api/src/context/ packages/api/src/index.ts
git commit -m "feat: add /context/for-file API endpoint"
```

---

## Task 2: CLI context Command

**Files:**
- Create: `apps/cli/src/commands/context-for.ts`
- Modify: `apps/cli/src/index.ts`

- [ ] **Step 1: Create the command**

```ts
// apps/cli/src/commands/context-for.ts
import { Command } from "commander";
import { getServerUrl } from "../lib/store";
import { output, outputError, isJson } from "../lib/output";
import { formatBold, formatDim, formatType } from "../lib/colors";

export const contextForCommand = new Command("context-for")
    .description("Generate context document for a file path")
    .argument("<path>", "file path to generate context for")
    .option("--codebase <name>", "scope to specific codebase")
    .option("--format <format>", "output format: markdown (default) or json", "markdown")
    .action(async (filePath: string, opts: { codebase?: string; format: string }, cmd: Command) => {
        let serverUrl: string;
        try {
            serverUrl = getServerUrl()!;
            if (!serverUrl) throw new Error();
        } catch {
            outputError("Server URL required. Run 'fubbik init' first.");
            return;
        }

        const params = new URLSearchParams({ path: filePath });
        if (opts.codebase) params.set("codebaseId", opts.codebase);
        params.set("format", opts.format);

        const res = await fetch(`${serverUrl}/api/context/for-file?${params}`);
        if (!res.ok) {
            outputError(`Failed to fetch context: ${res.statusText}`);
            return;
        }

        const chunks = await res.json();

        if (isJson(cmd)) {
            console.log(JSON.stringify(chunks, null, 2));
            return;
        }

        if (chunks.length === 0) {
            console.error(formatDim(`No chunks found for ${filePath}`));
            return;
        }

        // Output as markdown
        console.log(`# Context for ${filePath}`);
        console.log("");
        console.log(`> ${chunks.length} relevant chunk(s) found`);
        console.log("");

        for (const chunk of chunks) {
            console.log(`## ${chunk.title}`);
            console.log("");
            console.log(`**Type:** ${chunk.type} | **Match:** ${chunk.matchReason}`);
            console.log("");
            if (chunk.summary) {
                console.log(`> ${chunk.summary}`);
                console.log("");
            }
            console.log(chunk.content);
            console.log("");
            console.log("---");
            console.log("");
        }
    });
```

- [ ] **Step 2: Register in index.ts**

```ts
import { contextForCommand } from "./commands/context-for";
program.addCommand(contextForCommand);
```

- [ ] **Step 3: Commit**

```bash
git add apps/cli/src/commands/context-for.ts apps/cli/src/index.ts
git commit -m "feat(cli): add context-for command for AI assistant integration"
```
