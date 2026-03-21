# CLI Quality of Life Improvements Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Improve CLI usability with colored output, confirmation prompts, interactive mode, table formatting, template support, and shell completions.

**Architecture:** All changes are in `apps/cli/src/`. The CLI uses Commander.js for command parsing, a local JSON store for data, and native fetch for server communication. No backend changes needed.

**Tech Stack:** Commander.js, Bun, picocolors (new dep for colors), cli-table3 (new dep for tables)

---

## File Structure

### New files to create:
- `apps/cli/src/lib/colors.ts` — Color formatting utilities wrapping picocolors
- `apps/cli/src/lib/table.ts` — Table formatting utility wrapping cli-table3
- `apps/cli/src/lib/prompt.ts` — Interactive confirmation/input prompts using readline
- `apps/cli/src/__tests__/output.test.ts` — Tests for output formatting
- `apps/cli/src/__tests__/prompt.test.ts` — Tests for prompt utilities
- `apps/cli/src/__tests__/table.test.ts` — Tests for table formatting

### Files to modify:
- `apps/cli/package.json` — Add picocolors, cli-table3 dependencies
- `apps/cli/src/lib/output.ts` — Integrate color formatting
- `apps/cli/src/commands/remove.ts` — Add confirmation prompt
- `apps/cli/src/commands/add.ts` — Add interactive mode (-i flag)
- `apps/cli/src/commands/list.ts` — Table output format
- `apps/cli/src/commands/search.ts` — Table output format
- `apps/cli/src/commands/unlink.ts` — Add confirmation prompt
- `apps/cli/src/index.ts` — Register completion command

---

## Task 1: Add Color Support

Add colored terminal output using picocolors (zero-dependency, fast).

**Files:**
- Modify: `apps/cli/package.json`
- Create: `apps/cli/src/lib/colors.ts`
- Modify: `apps/cli/src/lib/output.ts`
- Create: `apps/cli/src/__tests__/output.test.ts`

- [ ] **Step 1: Install picocolors**

Run: `cd apps/cli && pnpm add picocolors`

- [ ] **Step 2: Write test for colored output helpers**

```ts
// apps/cli/src/__tests__/output.test.ts
import { describe, it, expect } from "vitest";
import { formatSuccess, formatError, formatDim, formatBold } from "../lib/colors";

describe("colors", () => {
  it("formatSuccess wraps text with green checkmark", () => {
    const result = formatSuccess("Created chunk abc");
    expect(result).toContain("Created chunk abc");
    expect(result).toContain("✓");
  });

  it("formatError wraps text with red cross", () => {
    const result = formatError("Not found");
    expect(result).toContain("Not found");
    expect(result).toContain("✗");
  });

  it("formatDim dims text", () => {
    const result = formatDim("metadata");
    expect(result).toBeDefined();
    expect(typeof result).toBe("string");
  });
});
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cd apps/cli && pnpm vitest run src/__tests__/output.test.ts`
Expected: FAIL — module not found

- [ ] **Step 4: Implement colors.ts**

```ts
// apps/cli/src/lib/colors.ts
import pc from "picocolors";

export const formatSuccess = (msg: string) => `${pc.green("✓")} ${msg}`;
export const formatError = (msg: string) => `${pc.red("✗")} ${msg}`;
export const formatDim = (msg: string) => pc.dim(msg);
export const formatBold = (msg: string) => pc.bold(msg);
export const formatType = (type: string) => pc.cyan(`[${type}]`);
export const formatId = (id: string) => pc.dim(id);
export const formatTag = (tag: string) => pc.yellow(tag);
export const formatTitle = (title: string) => pc.bold(title);
export const formatRelation = (rel: string) => pc.magenta(rel);
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cd apps/cli && pnpm vitest run src/__tests__/output.test.ts`
Expected: PASS

- [ ] **Step 6: Update output.ts to use colors**

In `output.ts`, replace plain `✓`/`✗` markers with `formatSuccess`/`formatError` from colors.ts. The `--json` and `--quiet` modes should remain uncolored.

- [ ] **Step 7: Update command files to use colored output**

Update the human-readable output in key commands:
- `add.ts`: Use `formatSuccess`, `formatType`, `formatTag` for creation message
- `remove.ts`: Use `formatSuccess` for deletion message
- `list.ts`: Use `formatType`, `formatTitle`, `formatDim` for each item
- `search.ts`: Use `formatTitle`, `formatDim` for results
- `link.ts`/`unlink.ts`: Use `formatRelation` for relation display

- [ ] **Step 8: Commit**

```bash
git add apps/cli/
git commit -m "feat(cli): add colored terminal output with picocolors"
```

---

## Task 2: Confirmation Prompts for Destructive Commands

Add interactive confirmation before `remove`, `unlink`, and bulk operations. Bypass with `--yes` flag.

**Files:**
- Create: `apps/cli/src/lib/prompt.ts`
- Create: `apps/cli/src/__tests__/prompt.test.ts`
- Modify: `apps/cli/src/commands/remove.ts`
- Modify: `apps/cli/src/commands/unlink.ts`

- [ ] **Step 1: Write test for confirm prompt**

```ts
// apps/cli/src/__tests__/prompt.test.ts
import { describe, it, expect } from "vitest";
import { parseConfirmInput } from "../lib/prompt";

describe("parseConfirmInput", () => {
  it("accepts y/Y/yes/YES as true", () => {
    expect(parseConfirmInput("y")).toBe(true);
    expect(parseConfirmInput("Y")).toBe(true);
    expect(parseConfirmInput("yes")).toBe(true);
    expect(parseConfirmInput("YES")).toBe(true);
  });

  it("rejects n/N/no/empty as false", () => {
    expect(parseConfirmInput("n")).toBe(false);
    expect(parseConfirmInput("N")).toBe(false);
    expect(parseConfirmInput("no")).toBe(false);
    expect(parseConfirmInput("")).toBe(false);
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd apps/cli && pnpm vitest run src/__tests__/prompt.test.ts`
Expected: FAIL

- [ ] **Step 3: Implement prompt.ts**

```ts
// apps/cli/src/lib/prompt.ts
import * as readline from "readline";

export function parseConfirmInput(input: string): boolean {
  return ["y", "yes"].includes(input.trim().toLowerCase());
}

export async function confirm(message: string): Promise<boolean> {
  const rl = readline.createInterface({ input: process.stdin, output: process.stderr });
  return new Promise((resolve) => {
    rl.question(`${message} [y/N] `, (answer) => {
      rl.close();
      resolve(parseConfirmInput(answer));
    });
  });
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd apps/cli && pnpm vitest run src/__tests__/prompt.test.ts`
Expected: PASS

- [ ] **Step 5: Add --yes flag and confirmation to remove.ts**

**Important:** The action handler must be changed to `async`. The current handler is synchronous. Also change opts type from `unknown` to `{ yes?: boolean }`.

```ts
// In remove.ts:
.option("-y, --yes", "skip confirmation prompt")

// Change action handler to async:
.action(async (id: string, opts: { yes?: boolean }, cmd: Command) => {
  const chunk = getChunk(id);
  if (!chunk) { outputError(`Chunk "${id}" not found.`); return; }

  if (!opts.yes) {
    const ok = await confirm(`Delete "${chunk.title}" (${id})? This cannot be undone.`);
    if (!ok) { console.error("Aborted."); return; }
  }
  deleteChunk(id);
  // ... existing output
});
```

- [ ] **Step 6: Add confirmation to unlink.ts**

**Note:** `unlink.ts` only receives a `connectionId` argument — source/target/relation are not available without a prior fetch. The confirmation message should be: `Delete connection ${connectionId}?`. If richer display is desired, add a GET request to fetch connection details first, but this requires a new API method. For now, keep it simple:

```ts
if (!opts.yes) {
  const ok = await confirm(`Delete connection "${connectionId}"?`);
  if (!ok) { console.error("Aborted."); return; }
}
```

- [ ] **Step 7: Commit**

```bash
git add apps/cli/src/lib/prompt.ts apps/cli/src/__tests__/prompt.test.ts apps/cli/src/commands/remove.ts apps/cli/src/commands/unlink.ts
git commit -m "feat(cli): add confirmation prompts for destructive commands"
```

---

## Task 3: Interactive Chunk Creation

Add `fubbik add -i` for guided, interactive chunk creation that opens `$EDITOR` for content.

**Files:**
- Modify: `apps/cli/src/commands/add.ts`
- Modify: `apps/cli/src/lib/prompt.ts` — add `promptInput` and `openEditor` functions

- [ ] **Step 1: Add promptInput and openEditor to prompt.ts**

```ts
// Append to prompt.ts
export async function promptInput(message: string, defaultValue = ""): Promise<string> {
  const rl = readline.createInterface({ input: process.stdin, output: process.stderr });
  const suffix = defaultValue ? ` (${defaultValue})` : "";
  return new Promise((resolve) => {
    rl.question(`${message}${suffix}: `, (answer) => {
      rl.close();
      resolve(answer.trim() || defaultValue);
    });
  });
}

export async function openEditor(initialContent = ""): Promise<string> {
  const { execFileSync } = await import("child_process");
  const { writeFileSync, readFileSync, unlinkSync } = await import("fs");
  const { tmpdir } = await import("os");
  const { join } = await import("path");

  const tmpFile = join(tmpdir(), `fubbik-${Date.now()}.md`);
  writeFileSync(tmpFile, initialContent);

  // Split EDITOR to handle cases like "code --wait" or paths with spaces
  const editor = process.env.EDITOR || process.env.VISUAL || "vi";
  const [cmd, ...args] = editor.split(" ");
  execFileSync(cmd, [...args, tmpFile], { stdio: "inherit" });

  const content = readFileSync(tmpFile, "utf-8");
  unlinkSync(tmpFile);
  return content;
}
```

- [ ] **Step 2: Add -i flag to add.ts**

```ts
.option("-i, --interactive", "interactive mode — prompts for each field")
```

- [ ] **Step 3: Implement interactive flow**

When `-i` is set:
```ts
if (opts.interactive) {
  const title = await promptInput("Title");
  const type = await promptInput("Type", "note");
  const tagsInput = await promptInput("Tags (comma-separated)", "");
  console.error("Opening editor for content...");
  const content = await openEditor(`# ${title}\n\n`);
  // ... create chunk with these values
}
```

- [ ] **Step 4: Test manually**

Run: `fubbik add -i`
Expected: Prompts for title, type, tags, then opens $EDITOR for content.

- [ ] **Step 5: Commit**

```bash
git add apps/cli/src/commands/add.ts apps/cli/src/lib/prompt.ts
git commit -m "feat(cli): add interactive chunk creation with -i flag"
```

---

## Task 4: Table Output for List and Search

Format `list` and `search` output as aligned tables.

**Files:**
- Modify: `apps/cli/package.json` — add cli-table3
- Create: `apps/cli/src/lib/table.ts`
- Create: `apps/cli/src/__tests__/table.test.ts`
- Modify: `apps/cli/src/commands/list.ts`
- Modify: `apps/cli/src/commands/search.ts`

- [ ] **Step 1: Install cli-table3**

Run: `cd apps/cli && pnpm add cli-table3 && pnpm add -D @types/cli-table3`

- [ ] **Step 2: Write test for table formatting**

```ts
// apps/cli/src/__tests__/table.test.ts
import { describe, it, expect } from "vitest";
import { formatChunkTable } from "../lib/table";

describe("formatChunkTable", () => {
  it("formats chunks into a table string", () => {
    const chunks = [
      { id: "c-abc", title: "My Chunk", type: "note", tags: ["api"], updatedAt: "2026-03-19" },
    ];
    const result = formatChunkTable(chunks);
    expect(result).toContain("My Chunk");
    expect(result).toContain("note");
    expect(result).toContain("c-abc");
  });

  it("handles empty array", () => {
    const result = formatChunkTable([]);
    expect(result).toContain("No chunks found");
  });
});
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cd apps/cli && pnpm vitest run src/__tests__/table.test.ts`
Expected: FAIL

- [ ] **Step 4: Implement table.ts**

```ts
// apps/cli/src/lib/table.ts
import Table from "cli-table3";
import { formatType, formatDim, formatTag } from "./colors";

interface ChunkRow {
  id: string;
  title: string;
  type: string;
  tags?: string[];
  updatedAt?: string;
}

export function formatChunkTable(chunks: ChunkRow[]): string {
  if (chunks.length === 0) return "No chunks found.";

  const table = new Table({
    head: ["ID", "Type", "Title", "Tags", "Updated"],
    colWidths: [12, 12, 40, 20, 12],
    wordWrap: true,
    style: { head: ["cyan"] },
  });

  for (const c of chunks) {
    table.push([
      c.id.slice(0, 10),
      c.type,
      c.title.length > 38 ? c.title.slice(0, 35) + "..." : c.title,
      (c.tags || []).join(", "),
      c.updatedAt ? new Date(c.updatedAt).toLocaleDateString() : "",
    ]);
  }

  return table.toString();
}
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cd apps/cli && pnpm vitest run src/__tests__/table.test.ts`
Expected: PASS

- [ ] **Step 6: Integrate into list.ts**

In the human-readable output path of `list.ts`, replace the newline-separated format with `formatChunkTable(chunks)`. Keep `--json` and `--quiet` modes unchanged.

- [ ] **Step 7: Integrate into search.ts**

Same pattern for search results. For semantic search, add a "Score" column to the table.

- [ ] **Step 8: Verify compiled binary works**

**Important:** `cli-table3` is CJS-only. While Bun handles CJS interop in dev mode, `bun build --compile` may have edge cases. After integration, verify the compiled binary:
```bash
cd apps/cli && bun build --compile src/index.ts --outfile dist/fubbik && ./dist/fubbik list
```

- [ ] **Step 9: Commit**

```bash
git add apps/cli/
git commit -m "feat(cli): format list and search output as tables"
```

---

## Task 5: Template Support in CLI

Expose server templates via `fubbik add --template <name>`.

**Files:**
- Modify: `apps/cli/src/commands/add.ts`

- [ ] **Step 1: Add --template flag to add.ts**

```ts
.option("--template <name>", "use a template (fetches from server)")
```

- [ ] **Step 2: Implement template fetching**

When `--template` is set. **Important:** `getServerUrl()` throws (not returns undefined) when no store exists. Must wrap in try/catch:
```ts
if (opts.template) {
  let serverUrl: string;
  try { serverUrl = getServerUrl(); } catch {
    outputError("Server URL required for templates. Run 'fubbik init'."); return;
  }

  const res = await fetch(`${serverUrl}/api/templates`);
  const templates = await res.json();
  const template = templates.find((t: any) =>
    t.name.toLowerCase() === opts.template.toLowerCase()
  );

  if (!template) {
    outputError(`Template "${opts.template}" not found. Available: ${templates.map((t: any) => t.name).join(", ")}`);
    return;
  }

  // Merge template fields with explicit flags (flags take precedence)
  title = opts.title || template.defaultTitle || "";
  content = content || template.content || "";
  type = opts.type || template.type || "note";
  tags = tags.length ? tags : (template.defaultTags || []);
}
```

- [ ] **Step 3: Add `fubbik templates` command to list available templates**

Quick subcommand that fetches and displays templates from the server:
```ts
// Could be a simple addition to index.ts or a new templates.ts command
program.command("templates")
  .description("List available templates")
  .action(async () => {
    const serverUrl = getServerUrl();
    const res = await fetch(`${serverUrl}/api/templates`);
    const templates = await res.json();
    // Format and display
  });
```

- [ ] **Step 4: Test manually**

Run: `fubbik templates` → should list available templates
Run: `fubbik add --template convention -t "My Convention"` → should pre-fill from template

- [ ] **Step 5: Commit**

```bash
git add apps/cli/src/commands/add.ts apps/cli/src/index.ts
git commit -m "feat(cli): add template support with --template flag"
```

---

## Task 6: Shell Completions

Generate zsh/bash completion scripts manually (Commander.js has no built-in completion support).

**Files:**
- Modify: `apps/cli/src/index.ts` — add completion generation

- [ ] **Step 1: Add completion command**

Commander.js doesn't have built-in completions, but we can generate a simple zsh completion script:

```ts
// In index.ts, add a hidden command:
program.command("completions")
  .description("Generate shell completions")
  .argument("<shell>", "shell type: zsh, bash, fish")
  .action((shell: string) => {
    if (shell === "zsh") {
      console.log(generateZshCompletions(program));
    } else if (shell === "bash") {
      console.log(generateBashCompletions(program));
    } else {
      console.error(`Unsupported shell: ${shell}. Use zsh or bash.`);
    }
  });
```

- [ ] **Step 2: Implement completion script generators**

Create `apps/cli/src/lib/completions.ts`:
```ts
import { Command } from "commander";

export function generateZshCompletions(program: Command): string {
  const commands = program.commands.map((cmd) => `"${cmd.name()}:${cmd.description()}"`).join("\n    ");
  return `#compdef fubbik
_fubbik() {
  local -a commands
  commands=(
    ${commands}
  )
  _describe 'command' commands
}
compdef _fubbik fubbik`;
}
```

- [ ] **Step 3: Document usage**

Output should include a comment at the top:
```
# Add to ~/.zshrc:
# eval "$(fubbik completions zsh)"
```

- [ ] **Step 4: Commit**

```bash
git add apps/cli/src/lib/completions.ts apps/cli/src/index.ts
git commit -m "feat(cli): add shell completion generation for zsh and bash"
```
