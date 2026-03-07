# CLI Agent Features Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make the fubbik CLI fully agent-friendly with machine-parseable output, file-based content input, pagination, new utility
commands, and connection management.

**Architecture:** All changes are in `apps/cli/`. We add a shared output helper (`lib/output.ts`) that every command uses — it checks for
`--json` and `--quiet` flags and formats accordingly. New commands (`cat`, `bulk-add`, `link`, `unlink`, `tags`, `stats`, `export`,
`import`, `diff`) follow the same pattern as existing ones. Store gets new helpers for connections, tags, and stats. No server or web
changes needed.

**Tech Stack:** TypeScript, Commander.js, Bun, Node fs

---

### Task 1: Shared Output Helper + Global Flags

Add `--json` and `--quiet` as global options, and a shared helper that all commands use for consistent output.

**Files:**

- Create: `apps/cli/src/lib/output.ts`
- Modify: `apps/cli/src/index.ts`

**Step 1: Create the output helper**

Create `apps/cli/src/lib/output.ts`:

```typescript
import type { Command } from "commander";

interface GlobalOpts {
    json?: boolean;
    quiet?: boolean;
}

function getGlobalOpts(cmd: Command): GlobalOpts {
    const root = cmd.parent ?? cmd;
    return root.opts() as GlobalOpts;
}

/** Print structured data. In --json mode: JSON. In --quiet mode: nothing. Otherwise: human text. */
export function output(cmd: Command, data: unknown, humanText: string): void {
    const opts = getGlobalOpts(cmd);
    if (opts.json) {
        console.log(JSON.stringify(data, null, 2));
    } else if (!opts.quiet) {
        console.log(humanText);
    }
}

/** Print just the essential value (e.g. an ID). Prints in all modes except --json (where output() handles it). */
export function outputQuiet(cmd: Command, value: string): void {
    const opts = getGlobalOpts(cmd);
    if (opts.quiet && !opts.json) {
        console.log(value);
    }
}

/** Print an error. Always prints to stderr regardless of flags. */
export function outputError(message: string): void {
    console.error(message);
}

export function isJson(cmd: Command): boolean {
    return getGlobalOpts(cmd).json === true;
}

export function isQuiet(cmd: Command): boolean {
    return getGlobalOpts(cmd).quiet === true;
}
```

**Step 2: Add global flags to index.ts**

Modify `apps/cli/src/index.ts`. Add global options to the program before adding commands:

Replace the current file with:

```typescript
import { Command } from "commander";

import { addCommand } from "./commands/add";
import { getCommand } from "./commands/get";
import { healthCommand } from "./commands/health";
import { initCommand } from "./commands/init";
import { listCommand } from "./commands/list";
import { removeCommand } from "./commands/remove";
import { searchCommand } from "./commands/search";
import { syncCommand } from "./commands/sync";
import { updateCommand } from "./commands/update";

const program = new Command();

program
    .name("fubbik")
    .description("A local-first knowledge framework for humans and machines")
    .version("0.0.1")
    .option("--json", "output as JSON (machine-readable)")
    .option("-q, --quiet", "minimal output (just IDs/values)");

program.addCommand(initCommand);
program.addCommand(healthCommand);
program.addCommand(addCommand);
program.addCommand(getCommand);
program.addCommand(listCommand);
program.addCommand(searchCommand);
program.addCommand(updateCommand);
program.addCommand(removeCommand);
program.addCommand(syncCommand);

program.parse();
```

**Step 3: Run type check**

Run: `cd /Users/pontus/GitHub/fubbik && bun run check-types`

**Step 4: Commit**

```bash
cd /Users/pontus/GitHub/fubbik && git add apps/cli/src/lib/output.ts apps/cli/src/index.ts
git commit -m "feat(cli): add shared output helper and global --json/--quiet flags"
```

---

### Task 2: Retrofit --json and --quiet on Existing Commands

Update all 9 existing commands to use the shared output helper. Remove per-command `--json` options (now global). Add `--quiet` support.

**Files:**

- Modify: `apps/cli/src/commands/add.ts`
- Modify: `apps/cli/src/commands/list.ts`
- Modify: `apps/cli/src/commands/get.ts`
- Modify: `apps/cli/src/commands/search.ts`
- Modify: `apps/cli/src/commands/update.ts`
- Modify: `apps/cli/src/commands/remove.ts`
- Modify: `apps/cli/src/commands/health.ts`
- Modify: `apps/cli/src/commands/sync.ts`
- Modify: `apps/cli/src/commands/init.ts`

**Step 1: Update add.ts**

Replace `apps/cli/src/commands/add.ts`:

```typescript
import { Command } from "commander";

import { addChunk } from "../lib/store";
import { isJson, output, outputError, outputQuiet } from "../lib/output";

export const addCommand = new Command("add")
    .description("Add a new chunk to the knowledge base")
    .requiredOption("-t, --title <title>", "chunk title")
    .option("-c, --content <content>", "chunk content", "")
    .option("--type <type>", "chunk type", "note")
    .option("--tags <tags>", "comma-separated tags", "")
    .option("--content-file <path>", "read content from file (use - for stdin)")
    .action(async (opts: { title: string; content: string; type: string; tags: string; contentFile?: string }, cmd: Command) => {
        let content = opts.content;
        if (opts.contentFile) {
            if (opts.contentFile === "-") {
                content = await Bun.stdin.text();
            } else {
                const { readFileSync } = await import("node:fs");
                content = readFileSync(opts.contentFile, "utf-8");
            }
        }

        const tags = opts.tags ? opts.tags.split(",").map(t => t.trim()) : [];
        const chunk = addChunk({ title: opts.title, content, type: opts.type, tags });

        outputQuiet(cmd, chunk.id);
        output(
            cmd,
            chunk,
            [
                `✓ Created chunk ${chunk.id}`,
                `  Title: ${chunk.title}`,
                `  Type: ${chunk.type}`,
                ...(tags.length > 0 ? [`  Tags: ${tags.join(", ")}`] : [])
            ].join("\n")
        );
    });
```

**Step 2: Update list.ts**

Replace `apps/cli/src/commands/list.ts`:

```typescript
import { Command } from "commander";

import { listChunks } from "../lib/store";
import { isJson, output, outputQuiet } from "../lib/output";

export const listCommand = new Command("list")
    .description("List all chunks")
    .option("--type <type>", "filter by type")
    .option("--tag <tag>", "filter by tag")
    .option("--limit <n>", "max number of results")
    .option("--offset <n>", "skip first n results")
    .option("--sort <field>", "sort by field (title, createdAt, updatedAt)", "updatedAt")
    .option("--sort-dir <dir>", "sort direction (asc, desc)", "desc")
    .option("--fields <fields>", "comma-separated fields to include (e.g. id,title)")
    .action(
        (
            opts: {
                type?: string;
                tag?: string;
                limit?: string;
                offset?: string;
                sort?: string;
                sortDir?: string;
                fields?: string;
            },
            cmd: Command
        ) => {
            let chunks = listChunks({ type: opts.type, tag: opts.tag });

            // Sort
            const sortField = opts.sort ?? "updatedAt";
            const sortDir = opts.sortDir === "asc" ? 1 : -1;
            chunks.sort((a, b) => {
                const aVal = String((a as Record<string, unknown>)[sortField] ?? "");
                const bVal = String((b as Record<string, unknown>)[sortField] ?? "");
                return aVal.localeCompare(bVal) * sortDir;
            });

            // Paginate
            const offset = Number(opts.offset) || 0;
            const limit = opts.limit ? Number(opts.limit) : undefined;
            if (offset > 0 || limit !== undefined) {
                chunks = chunks.slice(offset, limit !== undefined ? offset + limit : undefined);
            }

            // Field filter
            let data: unknown = chunks;
            if (opts.fields) {
                const fields = opts.fields.split(",").map(f => f.trim());
                data = chunks.map(c => {
                    const obj: Record<string, unknown> = {};
                    for (const f of fields) {
                        if (f in c) obj[f] = (c as Record<string, unknown>)[f];
                    }
                    return obj;
                });
            }

            outputQuiet(cmd, chunks.map(c => c.id).join("\n"));
            if (chunks.length === 0) {
                output(cmd, data, "No chunks found.");
            } else {
                const lines = [`${chunks.length} chunk(s):\n`];
                for (const chunk of chunks) {
                    const tags = chunk.tags.length > 0 ? ` [${chunk.tags.join(", ")}]` : "";
                    lines.push(`  ${chunk.id}  ${chunk.title}  (${chunk.type})${tags}`);
                }
                output(cmd, data, lines.join("\n"));
            }
        }
    );
```

**Step 3: Update get.ts**

Replace `apps/cli/src/commands/get.ts`:

```typescript
import { Command } from "commander";

import { getChunk } from "../lib/store";
import { output, outputError, outputQuiet } from "../lib/output";

export const getCommand = new Command("get")
    .description("Get a chunk by ID")
    .argument("<id>", "chunk ID")
    .action((id: string, _opts: unknown, cmd: Command) => {
        const chunk = getChunk(id);
        if (!chunk) {
            outputError(`✗ Chunk "${id}" not found.`);
            process.exit(1);
        }

        outputQuiet(cmd, chunk.id);
        const lines = [
            chunk.title,
            `  ID: ${chunk.id}`,
            `  Type: ${chunk.type}`,
            ...(chunk.tags.length > 0 ? [`  Tags: ${chunk.tags.join(", ")}`] : []),
            `  Created: ${chunk.createdAt}`,
            `  Updated: ${chunk.updatedAt}`,
            ...(chunk.content ? ["", chunk.content] : [])
        ];
        output(cmd, chunk, lines.join("\n"));
    });
```

**Step 4: Update search.ts**

Replace `apps/cli/src/commands/search.ts`:

```typescript
import { Command } from "commander";

import { searchChunks } from "../lib/store";
import { output, outputQuiet } from "../lib/output";

export const searchCommand = new Command("search")
    .description("Search chunks by title, content, or tags")
    .argument("<query>", "search query")
    .option("--limit <n>", "max number of results")
    .option("--offset <n>", "skip first n results")
    .option("--fields <fields>", "comma-separated fields to include")
    .action((query: string, opts: { limit?: string; offset?: string; fields?: string }, cmd: Command) => {
        let results = searchChunks(query);

        const offset = Number(opts.offset) || 0;
        const limit = opts.limit ? Number(opts.limit) : undefined;
        if (offset > 0 || limit !== undefined) {
            results = results.slice(offset, limit !== undefined ? offset + limit : undefined);
        }

        let data: unknown = results;
        if (opts.fields) {
            const fields = opts.fields.split(",").map(f => f.trim());
            data = results.map(c => {
                const obj: Record<string, unknown> = {};
                for (const f of fields) {
                    if (f in c) obj[f] = (c as Record<string, unknown>)[f];
                }
                return obj;
            });
        }

        outputQuiet(cmd, results.map(c => c.id).join("\n"));
        if (results.length === 0) {
            output(cmd, data, `No chunks matching "${query}".`);
        } else {
            const lines = [`${results.length} result(s) for "${query}":\n`];
            for (const chunk of results) {
                const tags = chunk.tags.length > 0 ? ` [${chunk.tags.join(", ")}]` : "";
                lines.push(`  ${chunk.id}  ${chunk.title}  (${chunk.type})${tags}`);
            }
            output(cmd, data, lines.join("\n"));
        }
    });
```

**Step 5: Update update.ts**

Replace `apps/cli/src/commands/update.ts`:

```typescript
import { Command } from "commander";

import { updateChunk } from "../lib/store";
import { output, outputError, outputQuiet } from "../lib/output";

export const updateCommand = new Command("update")
    .description("Update a chunk by ID")
    .argument("<id>", "chunk ID")
    .option("-t, --title <title>", "new title")
    .option("-c, --content <content>", "new content")
    .option("--type <type>", "new type")
    .option("--tags <tags>", "new comma-separated tags")
    .option("--content-file <path>", "read content from file (use - for stdin)")
    .action(
        async (
            id: string,
            opts: { title?: string; content?: string; type?: string; tags?: string; contentFile?: string },
            cmd: Command
        ) => {
            const updates: Record<string, unknown> = {};
            if (opts.title !== undefined) updates.title = opts.title;
            if (opts.type !== undefined) updates.type = opts.type;
            if (opts.tags !== undefined) updates.tags = opts.tags.split(",").map(t => t.trim());

            if (opts.contentFile) {
                if (opts.contentFile === "-") {
                    updates.content = await Bun.stdin.text();
                } else {
                    const { readFileSync } = await import("node:fs");
                    updates.content = readFileSync(opts.contentFile, "utf-8");
                }
            } else if (opts.content !== undefined) {
                updates.content = opts.content;
            }

            if (Object.keys(updates).length === 0) {
                outputError("✗ No updates provided. Use --title, --content, --type, --tags, or --content-file.");
                process.exit(1);
            }

            const chunk = updateChunk(id, updates);
            if (!chunk) {
                outputError(`✗ Chunk "${id}" not found.`);
                process.exit(1);
            }

            outputQuiet(cmd, chunk.id);
            output(
                cmd,
                chunk,
                [
                    `✓ Updated chunk ${chunk.id}`,
                    `  Title: ${chunk.title}`,
                    `  Type: ${chunk.type}`,
                    ...(chunk.tags.length > 0 ? [`  Tags: ${chunk.tags.join(", ")}`] : [])
                ].join("\n")
            );
        }
    );
```

**Step 6: Update remove.ts**

Replace `apps/cli/src/commands/remove.ts`:

```typescript
import { Command } from "commander";

import { deleteChunk, getChunk } from "../lib/store";
import { output, outputError, outputQuiet } from "../lib/output";

export const removeCommand = new Command("remove")
    .description("Remove a chunk by ID")
    .argument("<id>", "chunk ID")
    .action((id: string, _opts: unknown, cmd: Command) => {
        const chunk = getChunk(id);
        if (!chunk) {
            outputError(`✗ Chunk "${id}" not found.`);
            process.exit(1);
        }

        deleteChunk(id);
        outputQuiet(cmd, id);
        output(cmd, { id, title: chunk.title }, `✓ Removed chunk ${id} (${chunk.title})`);
    });
```

**Step 7: Update health.ts**

Replace `apps/cli/src/commands/health.ts`:

```typescript
import { Command } from "commander";

import { output, outputError } from "../lib/output";

const DEFAULT_URL = "http://localhost:3000";

export const healthCommand = new Command("health")
    .description("Check API server connection")
    .option("-u, --url <url>", "server URL", DEFAULT_URL)
    .action(async (opts: { url: string }, cmd: Command) => {
        try {
            const res = await fetch(opts.url);
            const body = await res.text();

            if (res.ok) {
                output(
                    cmd,
                    { status: res.status, url: opts.url, response: body },
                    [`✓ Connected to ${opts.url}`, `  Status: ${res.status}`, `  Response: ${body}`].join("\n")
                );
            } else {
                outputError(`✗ Server returned ${res.status}`);
                process.exit(1);
            }
        } catch (err) {
            outputError(`✗ Could not connect to ${opts.url}`);
            if (err instanceof Error) outputError(`  ${err.message}`);
            process.exit(1);
        }
    });
```

**Step 8: Update init.ts**

Replace `apps/cli/src/commands/init.ts`:

```typescript
import { Command } from "commander";

import { createStore, storeDir, storeExists } from "../lib/store";
import { output, outputError, outputQuiet } from "../lib/output";

export const initCommand = new Command("init")
    .description("Initialize a new knowledge base")
    .argument("[name]", "name for the knowledge base", "my-knowledge-base")
    .option("-f, --force", "overwrite existing knowledge base")
    .action((name: string, opts: { force?: boolean }, cmd: Command) => {
        if (storeExists() && !opts.force) {
            outputError("✗ Knowledge base already exists in this directory.");
            outputError("  Use --force to overwrite.");
            process.exit(1);
        }

        const store = createStore(name);
        outputQuiet(cmd, store.name);
        output(
            cmd,
            { name: store.name, location: storeDir() },
            [`✓ Initialized knowledge base: ${store.name}`, `  Location: ${storeDir()}`].join("\n")
        );
    });
```

**Step 9: Update sync.ts**

In `apps/cli/src/commands/sync.ts`, add `cmd` parameter and wrap the final summary in `output()`:

Replace the `.action(async (options: ...` line with:

```typescript
    .action(async (options: { url?: string; push?: boolean; pull?: boolean; token?: string }, cmd: Command) => {
```

Replace the final `console.log` lines (lines 43, 56, 73, 82, 97) to use the helpers:

- Keep `console.log` for progress messages (these are helpful even in JSON mode during a long operation)
- Wrap the final summary:

Replace lines 96-97:

```typescript
updateLastSync();
const summary = { pushed, pulled, localCount: store.chunks.length, serverCount: serverChunks.length };
output(cmd, summary, `Sync complete. Pushed: ${pushed}, Pulled: ${pulled}`);
```

Add imports at top:

```typescript
import { output, outputError } from "../lib/output";
```

Replace `console.error(...)` calls in error paths with `outputError(...)`.

**Step 10: Run type check**

Run: `cd /Users/pontus/GitHub/fubbik && bun run check-types`

**Step 11: Commit**

```bash
cd /Users/pontus/GitHub/fubbik && git add apps/cli/src/
git commit -m "feat(cli): retrofit --json/--quiet on all existing commands"
```

---

### Task 3: `cat` Command

Output only the raw content of a chunk — no metadata, no formatting. Perfect for piping.

**Files:**

- Create: `apps/cli/src/commands/cat.ts`
- Modify: `apps/cli/src/index.ts`

**Step 1: Create cat.ts**

Create `apps/cli/src/commands/cat.ts`:

```typescript
import { Command } from "commander";

import { getChunk } from "../lib/store";
import { outputError } from "../lib/output";

export const catCommand = new Command("cat")
    .description("Output raw content of a chunk (no metadata)")
    .argument("<id>", "chunk ID")
    .action((id: string) => {
        const chunk = getChunk(id);
        if (!chunk) {
            outputError(`✗ Chunk "${id}" not found.`);
            process.exit(1);
        }
        process.stdout.write(chunk.content);
    });
```

**Step 2: Register in index.ts**

Add import and registration to `apps/cli/src/index.ts`:

Add import:

```typescript
import { catCommand } from "./commands/cat";
```

Add after `program.addCommand(getCommand);`:

```typescript
program.addCommand(catCommand);
```

**Step 3: Run type check**

Run: `cd /Users/pontus/GitHub/fubbik && bun run check-types`

**Step 4: Commit**

```bash
cd /Users/pontus/GitHub/fubbik && git add apps/cli/src/commands/cat.ts apps/cli/src/index.ts
git commit -m "feat(cli): add cat command for raw content output"
```

---

### Task 4: `bulk-add` Command

Import chunks from a JSONL file (one JSON object per line).

**Files:**

- Create: `apps/cli/src/commands/bulk-add.ts`
- Modify: `apps/cli/src/index.ts`

**Step 1: Create bulk-add.ts**

Create `apps/cli/src/commands/bulk-add.ts`:

```typescript
import { readFileSync } from "node:fs";
import { Command } from "commander";

import { addChunk } from "../lib/store";
import { output, outputError, outputQuiet } from "../lib/output";

export const bulkAddCommand = new Command("bulk-add")
    .description("Import chunks from a JSONL file (one JSON object per line)")
    .requiredOption("--file <path>", "path to JSONL file (use - for stdin)")
    .action(async (opts: { file: string }, cmd: Command) => {
        let raw: string;
        if (opts.file === "-") {
            raw = await Bun.stdin.text();
        } else {
            raw = readFileSync(opts.file, "utf-8");
        }

        const lines = raw.split("\n").filter(l => l.trim());
        const added: { id: string; title: string }[] = [];
        const errors: { line: number; error: string }[] = [];

        for (let i = 0; i < lines.length; i++) {
            try {
                const obj = JSON.parse(lines[i]) as { title?: string; content?: string; type?: string; tags?: string[] };
                if (!obj.title) {
                    errors.push({ line: i + 1, error: "missing title" });
                    continue;
                }
                const chunk = addChunk({
                    title: obj.title,
                    content: obj.content ?? "",
                    type: obj.type ?? "note",
                    tags: obj.tags ?? []
                });
                added.push({ id: chunk.id, title: chunk.title });
            } catch (e) {
                errors.push({ line: i + 1, error: (e as Error).message });
            }
        }

        outputQuiet(cmd, added.map(a => a.id).join("\n"));
        output(
            cmd,
            { added, errors },
            [
                `✓ Added ${added.length} chunk(s)`,
                ...(errors.length > 0 ? [`✗ ${errors.length} error(s):`] : []),
                ...errors.map(e => `  Line ${e.line}: ${e.error}`)
            ].join("\n")
        );

        if (errors.length > 0) process.exit(1);
    });
```

**Step 2: Register in index.ts**

Add import and registration:

```typescript
import { bulkAddCommand } from "./commands/bulk-add";
```

```typescript
program.addCommand(bulkAddCommand);
```

**Step 3: Run type check**

Run: `cd /Users/pontus/GitHub/fubbik && bun run check-types`

**Step 4: Commit**

```bash
cd /Users/pontus/GitHub/fubbik && git add apps/cli/src/commands/bulk-add.ts apps/cli/src/index.ts
git commit -m "feat(cli): add bulk-add command for JSONL import"
```

---

### Task 5: `link` and `unlink` Commands

Manage connections between chunks from the CLI. These require a server connection since connections are stored server-side.

**Files:**

- Create: `apps/cli/src/commands/link.ts`
- Create: `apps/cli/src/commands/unlink.ts`
- Modify: `apps/cli/src/index.ts`

**Step 1: Create link.ts**

Create `apps/cli/src/commands/link.ts`:

```typescript
import { Command } from "commander";

import { getServerUrl, readStore } from "../lib/store";
import { output, outputError, outputQuiet } from "../lib/output";

export const linkCommand = new Command("link")
    .description("Create a connection between two chunks on the server")
    .argument("<source-id>", "source chunk ID")
    .argument("<target-id>", "target chunk ID")
    .option("-r, --relation <type>", "relation type", "related")
    .option("-u, --url <url>", "server URL")
    .option("--token <token>", "auth token")
    .action(async (sourceId: string, targetId: string, opts: { relation: string; url?: string; token?: string }, cmd: Command) => {
        const serverUrl = opts.url ?? getServerUrl();
        if (!serverUrl) {
            outputError("No server URL configured. Use --url or run sync once with --url.");
            process.exit(1);
        }

        const headers: Record<string, string> = { "Content-Type": "application/json" };
        if (opts.token) headers["Authorization"] = `Bearer ${opts.token}`;

        try {
            const res = await fetch(`${serverUrl}/api/connections`, {
                method: "POST",
                headers,
                body: JSON.stringify({ sourceId, targetId, relation: opts.relation })
            });

            if (!res.ok) {
                outputError(`✗ Failed to create connection: ${res.status} ${await res.text()}`);
                process.exit(1);
            }

            const data = await res.json();
            outputQuiet(cmd, (data as { id?: string }).id ?? "");
            output(cmd, data, `✓ Linked ${sourceId} → ${targetId} (${opts.relation})`);
        } catch (err) {
            outputError(`✗ ${(err as Error).message}`);
            process.exit(1);
        }
    });
```

**Step 2: Create unlink.ts**

Create `apps/cli/src/commands/unlink.ts`:

```typescript
import { Command } from "commander";

import { getServerUrl } from "../lib/store";
import { output, outputError, outputQuiet } from "../lib/output";

export const unlinkCommand = new Command("unlink")
    .description("Remove a connection by ID from the server")
    .argument("<connection-id>", "connection ID")
    .option("-u, --url <url>", "server URL")
    .option("--token <token>", "auth token")
    .action(async (connectionId: string, opts: { url?: string; token?: string }, cmd: Command) => {
        const serverUrl = opts.url ?? getServerUrl();
        if (!serverUrl) {
            outputError("No server URL configured. Use --url or run sync once with --url.");
            process.exit(1);
        }

        const headers: Record<string, string> = {};
        if (opts.token) headers["Authorization"] = `Bearer ${opts.token}`;

        try {
            const res = await fetch(`${serverUrl}/api/connections/${connectionId}`, {
                method: "DELETE",
                headers
            });

            if (!res.ok) {
                outputError(`✗ Failed to delete connection: ${res.status}`);
                process.exit(1);
            }

            outputQuiet(cmd, connectionId);
            output(cmd, { id: connectionId }, `✓ Removed connection ${connectionId}`);
        } catch (err) {
            outputError(`✗ ${(err as Error).message}`);
            process.exit(1);
        }
    });
```

**Step 3: Register in index.ts**

Add imports and registrations:

```typescript
import { linkCommand } from "./commands/link";
import { unlinkCommand } from "./commands/unlink";
```

```typescript
program.addCommand(linkCommand);
program.addCommand(unlinkCommand);
```

**Step 4: Run type check**

Run: `cd /Users/pontus/GitHub/fubbik && bun run check-types`

**Step 5: Commit**

```bash
cd /Users/pontus/GitHub/fubbik && git add apps/cli/src/commands/link.ts apps/cli/src/commands/unlink.ts apps/cli/src/index.ts
git commit -m "feat(cli): add link and unlink commands for connection management"
```

---

### Task 6: `tags` Command

List all unique tags with counts.

**Files:**

- Create: `apps/cli/src/commands/tags.ts`
- Modify: `apps/cli/src/index.ts`

**Step 1: Create tags.ts**

Create `apps/cli/src/commands/tags.ts`:

```typescript
import { Command } from "commander";

import { readStore } from "../lib/store";
import { output, outputQuiet } from "../lib/output";

export const tagsCommand = new Command("tags").description("List all unique tags with counts").action((_opts: unknown, cmd: Command) => {
    const store = readStore();
    const counts = new Map<string, number>();

    for (const chunk of store.chunks) {
        for (const tag of chunk.tags) {
            counts.set(tag, (counts.get(tag) ?? 0) + 1);
        }
    }

    const sorted = [...counts.entries()].sort((a, b) => b[1] - a[1]);
    const data = sorted.map(([tag, count]) => ({ tag, count }));

    outputQuiet(cmd, sorted.map(([tag]) => tag).join("\n"));

    if (sorted.length === 0) {
        output(cmd, data, "No tags found.");
    } else {
        const lines = [`${sorted.length} tag(s):\n`];
        for (const [tag, count] of sorted) {
            lines.push(`  ${tag} (${count})`);
        }
        output(cmd, data, lines.join("\n"));
    }
});
```

**Step 2: Register in index.ts**

```typescript
import { tagsCommand } from "./commands/tags";
```

```typescript
program.addCommand(tagsCommand);
```

**Step 3: Run type check**

Run: `cd /Users/pontus/GitHub/fubbik && bun run check-types`

**Step 4: Commit**

```bash
cd /Users/pontus/GitHub/fubbik && git add apps/cli/src/commands/tags.ts apps/cli/src/index.ts
git commit -m "feat(cli): add tags command to list unique tags with counts"
```

---

### Task 7: `stats` Command

Print summary statistics about the knowledge base.

**Files:**

- Create: `apps/cli/src/commands/stats.ts`
- Modify: `apps/cli/src/index.ts`

**Step 1: Create stats.ts**

Create `apps/cli/src/commands/stats.ts`:

```typescript
import { Command } from "commander";

import { readStore } from "../lib/store";
import { output } from "../lib/output";

export const statsCommand = new Command("stats").description("Show knowledge base statistics").action((_opts: unknown, cmd: Command) => {
    const store = readStore();
    const chunks = store.chunks;

    const typeCounts = new Map<string, number>();
    const tagCounts = new Map<string, number>();

    for (const chunk of chunks) {
        typeCounts.set(chunk.type, (typeCounts.get(chunk.type) ?? 0) + 1);
        for (const tag of chunk.tags) {
            tagCounts.set(tag, (tagCounts.get(tag) ?? 0) + 1);
        }
    }

    const data = {
        name: store.name,
        totalChunks: chunks.length,
        types: Object.fromEntries(typeCounts),
        uniqueTags: tagCounts.size,
        topTags: [...tagCounts.entries()]
            .sort((a, b) => b[1] - a[1])
            .slice(0, 5)
            .map(([tag, count]) => ({ tag, count })),
        lastSync: store.lastSync ?? null,
        serverUrl: store.serverUrl ?? null
    };

    const lines = [
        `Knowledge base: ${store.name}`,
        `Total chunks: ${chunks.length}`,
        "",
        "Types:",
        ...[...typeCounts.entries()].map(([type, count]) => `  ${type}: ${count}`),
        "",
        `Unique tags: ${tagCounts.size}`,
        ...(data.topTags.length > 0 ? ["Top tags:", ...data.topTags.map(t => `  ${t.tag} (${t.count})`)] : []),
        "",
        `Last sync: ${store.lastSync ?? "never"}`,
        `Server: ${store.serverUrl ?? "not configured"}`
    ];

    output(cmd, data, lines.join("\n"));
});
```

**Step 2: Register in index.ts**

```typescript
import { statsCommand } from "./commands/stats";
```

```typescript
program.addCommand(statsCommand);
```

**Step 3: Run type check**

Run: `cd /Users/pontus/GitHub/fubbik && bun run check-types`

**Step 4: Commit**

```bash
cd /Users/pontus/GitHub/fubbik && git add apps/cli/src/commands/stats.ts apps/cli/src/index.ts
git commit -m "feat(cli): add stats command for knowledge base overview"
```

---

### Task 8: `export` Command

Export the entire knowledge base as JSON or markdown files.

**Files:**

- Create: `apps/cli/src/commands/export.ts`
- Modify: `apps/cli/src/index.ts`

**Step 1: Create export.ts**

Create `apps/cli/src/commands/export.ts`:

```typescript
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { Command } from "commander";

import { readStore } from "../lib/store";
import { output, outputError } from "../lib/output";

export const exportCommand = new Command("export")
    .description("Export the knowledge base")
    .option("--format <format>", "output format: json or md", "json")
    .option("--out <dir>", "output directory for md format", "export")
    .action((_opts: { format: string; out: string }, cmd: Command) => {
        const store = readStore();

        if (_opts.format === "json") {
            output(cmd, store.chunks, JSON.stringify(store.chunks, null, 2));
        } else if (_opts.format === "md") {
            mkdirSync(_opts.out, { recursive: true });
            for (const chunk of store.chunks) {
                const frontmatter = [
                    "---",
                    `id: ${chunk.id}`,
                    `title: "${chunk.title.replace(/"/g, '\\"')}"`,
                    `type: ${chunk.type}`,
                    `tags: [${chunk.tags.map(t => `"${t}"`).join(", ")}]`,
                    `createdAt: ${chunk.createdAt}`,
                    `updatedAt: ${chunk.updatedAt}`,
                    "---",
                    "",
                    `# ${chunk.title}`,
                    "",
                    chunk.content
                ].join("\n");

                const filename = `${chunk.id}.md`;
                writeFileSync(join(_opts.out, filename), frontmatter);
            }
            output(cmd, { count: store.chunks.length, dir: _opts.out }, `✓ Exported ${store.chunks.length} chunk(s) to ${_opts.out}/`);
        } else {
            outputError(`✗ Unknown format "${_opts.format}". Use json or md.`);
            process.exit(1);
        }
    });
```

**Step 2: Register in index.ts**

```typescript
import { exportCommand } from "./commands/export";
```

```typescript
program.addCommand(exportCommand);
```

**Step 3: Run type check**

Run: `cd /Users/pontus/GitHub/fubbik && bun run check-types`

**Step 4: Commit**

```bash
cd /Users/pontus/GitHub/fubbik && git add apps/cli/src/commands/export.ts apps/cli/src/index.ts
git commit -m "feat(cli): add export command (json and markdown formats)"
```

---

### Task 9: `import` Command

Import chunks from a JSON array file or a directory of markdown files with YAML frontmatter.

**Files:**

- Create: `apps/cli/src/commands/import.ts`
- Modify: `apps/cli/src/index.ts`

**Step 1: Create import.ts**

Create `apps/cli/src/commands/import.ts`:

```typescript
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";
import { Command } from "commander";

import { addChunk } from "../lib/store";
import { output, outputError, outputQuiet } from "../lib/output";

function parseFrontmatter(raw: string): { meta: Record<string, string>; content: string } {
    const match = raw.match(/^---\n([\s\S]*?)\n---\n([\s\S]*)$/);
    if (!match) return { meta: {}, content: raw };

    const meta: Record<string, string> = {};
    for (const line of match[1].split("\n")) {
        const idx = line.indexOf(":");
        if (idx > 0) {
            meta[line.slice(0, idx).trim()] = line.slice(idx + 1).trim();
        }
    }
    // Strip leading "# Title" line from content
    let content = match[2];
    const titleLine = content.match(/^# .+\n\n?/);
    if (titleLine) content = content.slice(titleLine[0].length);

    return { meta, content };
}

function parseTags(raw: string): string[] {
    const match = raw.match(/\[(.+)\]/);
    if (!match) return [];
    return match[1].split(",").map(t => t.trim().replace(/^"|"$/g, ""));
}

export const importCommand = new Command("import")
    .description("Import chunks from a JSON file or markdown directory")
    .requiredOption("--file <path>", "path to JSON file or directory of .md files")
    .action((opts: { file: string }, cmd: Command) => {
        const stat = statSync(opts.file);
        const added: { id: string; title: string }[] = [];

        if (stat.isDirectory()) {
            // Import markdown files
            const files = readdirSync(opts.file).filter(f => f.endsWith(".md"));
            for (const file of files) {
                const raw = readFileSync(join(opts.file, file), "utf-8");
                const { meta, content } = parseFrontmatter(raw);
                const title = meta.title?.replace(/^"|"$/g, "") ?? file.replace(".md", "");
                const chunk = addChunk({
                    title,
                    content: content.trim(),
                    type: meta.type ?? "note",
                    tags: meta.tags ? parseTags(meta.tags) : []
                });
                added.push({ id: chunk.id, title: chunk.title });
            }
        } else {
            // Import JSON array
            const raw = readFileSync(opts.file, "utf-8");
            const chunks = JSON.parse(raw) as Array<{ title: string; content?: string; type?: string; tags?: string[] }>;
            for (const obj of chunks) {
                if (!obj.title) continue;
                const chunk = addChunk({
                    title: obj.title,
                    content: obj.content ?? "",
                    type: obj.type ?? "note",
                    tags: obj.tags ?? []
                });
                added.push({ id: chunk.id, title: chunk.title });
            }
        }

        outputQuiet(cmd, added.map(a => a.id).join("\n"));
        output(cmd, { added }, `✓ Imported ${added.length} chunk(s)`);
    });
```

**Step 2: Register in index.ts**

```typescript
import { importCommand } from "./commands/import";
```

```typescript
program.addCommand(importCommand);
```

**Step 3: Run type check**

Run: `cd /Users/pontus/GitHub/fubbik && bun run check-types`

**Step 4: Commit**

```bash
cd /Users/pontus/GitHub/fubbik && git add apps/cli/src/commands/import.ts apps/cli/src/index.ts
git commit -m "feat(cli): add import command (json and markdown directory)"
```

---

### Task 10: `diff` Command

Show what changed since last sync (new local, new remote, modified).

**Files:**

- Create: `apps/cli/src/commands/diff.ts`
- Modify: `apps/cli/src/index.ts`

**Step 1: Create diff.ts**

Create `apps/cli/src/commands/diff.ts`:

```typescript
import { Command } from "commander";

import { getServerUrl, readStore } from "../lib/store";
import { output, outputError } from "../lib/output";

export const diffCommand = new Command("diff")
    .description("Show differences between local and server chunks")
    .option("-u, --url <url>", "server URL")
    .option("--token <token>", "auth token")
    .action(async (opts: { url?: string; token?: string }, cmd: Command) => {
        const store = readStore();
        const serverUrl = opts.url ?? getServerUrl();
        if (!serverUrl) {
            outputError("No server URL configured. Use --url or run sync once with --url.");
            process.exit(1);
        }

        const headers: Record<string, string> = {};
        if (opts.token) headers["Authorization"] = `Bearer ${opts.token}`;

        try {
            const res = await fetch(`${serverUrl}/api/chunks?limit=1000`, { headers });
            if (!res.ok) {
                outputError(`✗ Server returned ${res.status}`);
                process.exit(1);
            }

            const serverData = (await res.json()) as { chunks: Array<{ id: string; title: string; updatedAt: string }> };
            const serverChunks = serverData.chunks;

            const localByTitle = new Map(store.chunks.map(c => [c.title, c]));
            const serverByTitle = new Map(serverChunks.map(c => [c.title, c]));

            const localOnly = store.chunks.filter(c => !serverByTitle.has(c.title));
            const serverOnly = serverChunks.filter(c => !localByTitle.has(c.title));
            const modified = store.chunks.filter(c => {
                const sc = serverByTitle.get(c.title);
                return sc && sc.updatedAt !== c.updatedAt;
            });

            const data = {
                localOnly: localOnly.map(c => ({ id: c.id, title: c.title })),
                serverOnly: serverOnly.map(c => ({ id: c.id, title: c.title })),
                modified: modified.map(c => ({ id: c.id, title: c.title })),
                lastSync: store.lastSync ?? null
            };

            const lines = [`Last sync: ${store.lastSync ?? "never"}`, `Local: ${store.chunks.length}, Server: ${serverChunks.length}`, ""];

            if (localOnly.length > 0) {
                lines.push(`Local only (${localOnly.length}):`);
                for (const c of localOnly) lines.push(`  + ${c.id}  ${c.title}`);
                lines.push("");
            }
            if (serverOnly.length > 0) {
                lines.push(`Server only (${serverOnly.length}):`);
                for (const c of serverOnly) lines.push(`  - ${c.id}  ${c.title}`);
                lines.push("");
            }
            if (modified.length > 0) {
                lines.push(`Modified (${modified.length}):`);
                for (const c of modified) lines.push(`  ~ ${c.id}  ${c.title}`);
                lines.push("");
            }
            if (localOnly.length === 0 && serverOnly.length === 0 && modified.length === 0) {
                lines.push("No differences found.");
            }

            output(cmd, data, lines.join("\n"));
        } catch (err) {
            outputError(`✗ ${(err as Error).message}`);
            process.exit(1);
        }
    });
```

**Step 2: Register in index.ts**

```typescript
import { diffCommand } from "./commands/diff";
```

```typescript
program.addCommand(diffCommand);
```

**Step 3: Run type check**

Run: `cd /Users/pontus/GitHub/fubbik && bun run check-types`

**Step 4: Commit**

```bash
cd /Users/pontus/GitHub/fubbik && git add apps/cli/src/commands/diff.ts apps/cli/src/index.ts
git commit -m "feat(cli): add diff command to show local vs server differences"
```
