# CLI Improvements Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix CLI crashes and add quality-of-life improvements for a more robust developer experience.

**Architecture:** All changes are in `apps/cli/src/`. Bug fixes modify `lib/store.ts` and existing commands. New `status` command composes existing APIs.

**Tech Stack:** Commander.js, picocolors, bun, fetch API.

**Note:** Item 6 (stdin support for `add`) is already implemented via `--content-file -`. Skipped.

---

### Task 1: Fix `getServerUrl()` crashing without init

**Files:**
- Modify: `apps/cli/src/lib/store.ts`

The root cause: `getServerUrl()` calls `readStore()` which throws if `.fubbik/store.json` doesn't exist. Every command that calls `getServerUrl()` crashes with a stack trace.

- [ ] **Step 1: Make `getServerUrl` return `undefined` gracefully**

In `apps/cli/src/lib/store.ts`, change `getServerUrl` (line 116-119):

```typescript
export function getServerUrl(dir?: string): string | undefined {
    try {
        const store = readStore(dir);
        return store.serverUrl;
    } catch {
        return undefined;
    }
}
```

- [ ] **Step 2: Verify fix**

Run: `cd /tmp && fubbik list 2>&1` (outside any fubbik project)
Expected: Should show a helpful error ("No server URL configured"), not a stack trace.

Run: `cd /Users/pontus/projects/fubbik && fubbik recap --since 7d`
Expected: Should work as before.

- [ ] **Step 3: Commit**

```bash
git add apps/cli/src/lib/store.ts
git commit -m "fix: getServerUrl returns undefined gracefully without store file"
```

---

### Task 2: Fix `stats` crash on null tags

**Files:**
- Modify: `apps/cli/src/commands/stats.ts`

Chunks pulled from server may have `undefined` tags. Line 15 iterates `chunk.tags` without a null guard.

- [ ] **Step 1: Add null guard**

In `apps/cli/src/commands/stats.ts`, change line 15:

```typescript
for (const tag of chunk.tags ?? []) {
```

- [ ] **Step 2: Verify fix**

Run: `fubbik stats`
Expected: Shows stats without crash.

- [ ] **Step 3: Commit**

```bash
git add apps/cli/src/commands/stats.ts
git commit -m "fix: guard against null tags in stats command"
```

---

### Task 3: Fix `gaps` working directory resolution

**Files:**
- Modify: `apps/cli/src/commands/gaps.ts`

The `gaps` command resolves paths relative to where the binary is, not `process.cwd()`. The issue is on line 50: `join(process.cwd(), directory)` — this works in dev (`bun apps/cli/src/index.ts`) but the compiled binary may resolve differently.

- [ ] **Step 1: Fix path resolution**

In `apps/cli/src/commands/gaps.ts`, change the path resolution (line 50-51):

```typescript
import { resolve } from "node:path";

// Replace:
const absDir = join(process.cwd(), directory);
const allFiles = collectSourceFiles(absDir, process.cwd());

// With:
const absDir = resolve(directory);
const allFiles = collectSourceFiles(absDir, absDir);
```

This ensures: `fubbik gaps packages/api/src` resolves relative to cwd, and all collected file paths are relative to the scanned directory (not cwd).

- [ ] **Step 2: Verify fix**

Run: `fubbik gaps packages/api/src --limit 5`
Expected: Shows files relative to `packages/api/src/`.

- [ ] **Step 3: Commit**

```bash
git add apps/cli/src/commands/gaps.ts
git commit -m "fix: resolve gaps directory relative to cwd correctly"
```

---

### Task 4: Add `--server` option to `fubbik init`

**Files:**
- Modify: `apps/cli/src/commands/init.ts`

Currently you must `init` then `sync --url` separately. Add a `--server <url>` option.

- [ ] **Step 1: Add the option**

In `apps/cli/src/commands/init.ts`, add option after line 16:

```typescript
.option("--server <url>", "configure server URL during init")
```

Update the action signature to include `server?: string`.

After `createStore(name)` (around line 27), add:

```typescript
if (opts.server) {
    setServerUrl(opts.server);
    console.log(`  Server: ${opts.server}`);
}
```

Add `setServerUrl` to the imports from `../lib/store`.

- [ ] **Step 2: Verify**

Run: `rm -rf .fubbik && fubbik init fubbik --server http://localhost:3000 && fubbik health`
Expected: Init succeeds and health check connects.

- [ ] **Step 3: Commit**

```bash
git add apps/cli/src/commands/init.ts
git commit -m "feat: add --server option to fubbik init"
```

---

### Task 5: Add `fubbik status` command

**Files:**
- Create: `apps/cli/src/commands/status.ts`
- Modify: `apps/cli/src/index.ts`

A single command showing: server connection, codebase, chunk count, last sync, and health warnings. Like `git status` for the knowledge base.

- [ ] **Step 1: Create status command**

Create `apps/cli/src/commands/status.ts`:

```typescript
import { Command } from "commander";
import { formatDim, formatError, formatSuccess, formatTag, formatType } from "../lib/colors";
import { output } from "../lib/output";
import { getServerUrl, readStore, storeExists } from "../lib/store";
import { detectCodebase } from "../lib/detect-codebase";

export const statusCommand = new Command("status")
    .description("Show knowledge base status overview")
    .action(async (_opts: unknown, cmd: Command) => {
        const lines: string[] = [];
        const data: Record<string, unknown> = {};

        // Store status
        if (!storeExists()) {
            lines.push(formatError("No local store. Run 'fubbik init' to get started."));
            output(cmd, { initialized: false }, lines.join("\n"));
            return;
        }

        const store = readStore();
        data.name = store.name;
        data.localChunks = store.chunks.length;
        data.lastSync = store.lastSync ?? null;
        data.serverUrl = store.serverUrl ?? null;

        lines.push(`Knowledge base: ${store.name}`);
        lines.push(`  Local chunks: ${store.chunks.length}`);
        lines.push(`  Last sync: ${store.lastSync ? new Date(store.lastSync).toLocaleString() : formatDim("never")}`);

        // Server connection
        const serverUrl = getServerUrl();
        if (!serverUrl) {
            lines.push(`  Server: ${formatDim("not configured")}`);
        } else {
            lines.push(`  Server: ${serverUrl}`);
            try {
                const healthRes = await fetch(`${serverUrl}/api/health`);
                if (healthRes.ok) {
                    lines.push(`  Connection: ${formatSuccess("connected")}`);
                    data.serverConnected = true;

                    // Fetch server stats
                    try {
                        const statsRes = await fetch(`${serverUrl}/api/stats`);
                        if (statsRes.ok) {
                            const stats = (await statsRes.json()) as { chunks: number; connections: number; tags: number };
                            lines.push(`  Server chunks: ${stats.chunks}`);
                            lines.push(`  Connections: ${stats.connections}`);
                            lines.push(`  Tags: ${stats.tags}`);
                            data.serverStats = stats;
                        }
                    } catch {
                        // stats endpoint may require auth
                    }
                } else {
                    lines.push(`  Connection: ${formatError("unhealthy")} (${healthRes.status})`);
                    data.serverConnected = false;
                }
            } catch {
                lines.push(`  Connection: ${formatError("unreachable")}`);
                data.serverConnected = false;
            }
        }

        // Codebase detection
        const codebase = await detectCodebase();
        if (codebase) {
            lines.push(`  Codebase: ${codebase.name} ${formatDim(`(${codebase.id.slice(0, 8)})`)}`);
            data.codebase = codebase;
        } else {
            lines.push(`  Codebase: ${formatDim("not detected")}`);
        }

        // Health warnings (quick check)
        if (serverUrl && data.serverConnected) {
            try {
                const healthRes = await fetch(`${serverUrl}/api/health/knowledge`);
                if (healthRes.ok) {
                    const health = (await healthRes.json()) as {
                        orphanChunks?: number;
                        staleChunks?: number;
                        thinChunks?: number;
                        staleEmbeddings?: number;
                    };
                    const warnings: string[] = [];
                    if (health.orphanChunks) warnings.push(`${health.orphanChunks} orphan`);
                    if (health.staleChunks) warnings.push(`${health.staleChunks} stale`);
                    if (health.thinChunks) warnings.push(`${health.thinChunks} thin`);
                    if (health.staleEmbeddings) warnings.push(`${health.staleEmbeddings} stale embeddings`);

                    if (warnings.length > 0) {
                        lines.push("");
                        lines.push(`  Warnings: ${warnings.join(", ")}`);
                        lines.push(`  Run 'fubbik lint' for details`);
                    }
                    data.health = health;
                }
            } catch {
                // health endpoint may require auth
            }
        }

        output(cmd, data, lines.join("\n"));
    });
```

- [ ] **Step 2: Register the command**

In `apps/cli/src/index.ts`, add import:
```typescript
import { statusCommand } from "./commands/status";
```

Add after `program.addCommand(statsCommand);`:
```typescript
program.addCommand(statusCommand);
```

- [ ] **Step 3: Verify**

Run: `fubbik status`
Expected: Shows name, local chunks, server connection, codebase, health warnings.

- [ ] **Step 4: Commit**

```bash
git add apps/cli/src/commands/status.ts apps/cli/src/index.ts
git commit -m "feat: add fubbik status command for KB overview"
```

---

### Task 6: Add colored output to existing commands

**Files:**
- Modify: `apps/cli/src/commands/recap.ts`
- Modify: `apps/cli/src/commands/why.ts`
- Modify: `apps/cli/src/commands/gaps.ts`

The three new commands output plain text. Add colors using the existing `picocolors` helpers from `../lib/colors`.

- [ ] **Step 1: Add colors to recap**

In `apps/cli/src/commands/recap.ts`, import colors:
```typescript
import { formatBold, formatDim, formatType } from "../lib/colors";
```

Update the output formatting:
- Wrap section headers ("New:", "Updated:", "By type:") with `formatBold()`
- Wrap type labels with `formatType()`
- Wrap counts with `formatDim()` for secondary info

- [ ] **Step 2: Add colors to why**

In `apps/cli/src/commands/why.ts`, import and apply:
- Wrap the file path with `formatBold()`
- Wrap type labels with `formatType()`
- Wrap "Rationale:" and "Alternatives:" labels with `formatDim()`
- Wrap match reasons with `formatTag()`

- [ ] **Step 3: Add colors to gaps**

In `apps/cli/src/commands/gaps.ts`, import and apply:
- Color the coverage percentage: green if >75%, yellow if >50%, red otherwise
- Wrap directory names with `formatBold()`
- Wrap file names with `formatDim()`

- [ ] **Step 4: Verify all three**

Run: `fubbik recap --since 30d && fubbik why packages/api/src/index.ts && fubbik gaps packages/api/src --limit 5`
Expected: Colored output for all three commands.

- [ ] **Step 5: Commit**

```bash
git add apps/cli/src/commands/recap.ts apps/cli/src/commands/why.ts apps/cli/src/commands/gaps.ts
git commit -m "feat: add colored output to recap, why, and gaps commands"
```

---

### Task 7: Rebuild and verify compiled binary

**Files:**
- No code changes — just build and test

- [ ] **Step 1: Rebuild CLI**

Run: `cd apps/cli && pnpm build`

- [ ] **Step 2: Test all fixed commands**

```bash
fubbik status
fubbik stats
fubbik recap --since 7d
fubbik why packages/api/src/chunks/service.ts
fubbik gaps packages/api/src --limit 5
fubbik list --limit 3
```

Expected: All commands work without stack traces.

- [ ] **Step 3: Test edge cases**

```bash
# Outside a fubbik project
cd /tmp && fubbik status
# Expected: Shows "not configured" gracefully, no crash

# With --json flag
fubbik status --json
# Expected: Valid JSON output
```
