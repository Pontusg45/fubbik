# CLI Restructuring Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reduce the fubbik CLI from 49 top-level commands to ~30 by grouping related commands into namespaces, extract shared API helpers, and fix output contract violations.

**Architecture:** Three phases — (1) extract shared `lib/api.ts` and fix output contract in review.ts/plan.ts, (2) create 5 group command files that compose existing subcommands under `chunk`, `context`, `tag`, `req`, `maintain`, (3) rewire `index.ts` to register groups instead of individual commands, update tests.

**Tech Stack:** Commander.js, TypeScript, Vitest

**Spec:** `docs/superpowers/specs/2026-04-12-cli-restructuring-design.md`

---

## File Structure

### Created

| Path | Responsibility |
|---|---|
| `apps/cli/src/lib/api.ts` | Shared `requireServer`, `fetchApi`, `fetchApiJson` |
| `apps/cli/src/commands/chunk.ts` | Group: 12 chunk subcommands |
| `apps/cli/src/commands/context-group.ts` | Group: 3 context subcommands |
| `apps/cli/src/commands/tag-group.ts` | Group: tags + normalize |
| `apps/cli/src/commands/req.ts` | Group: requirements + import |
| `apps/cli/src/commands/maintain.ts` | Group: doctor, cleanup, lint, health, seed-conventions |

### Modified

| Path | Change |
|---|---|
| `apps/cli/src/commands/review.ts` | Use shared `lib/api`, fix output contract |
| `apps/cli/src/commands/plan.ts` | Use shared `lib/api` |
| `apps/cli/src/commands/task.ts` | Use shared `lib/api` |
| `apps/cli/src/commands/context.ts` | Add `.name("export")` |
| `apps/cli/src/commands/context-dir.ts` | Add `.name("dir")` |
| `apps/cli/src/commands/context-for.ts` | Add `.name("for")` |
| `apps/cli/src/commands/tag-normalize.ts` | Add `.name("normalize")` |
| `apps/cli/src/commands/import-requirements.ts` | Add `.name("import")` |
| `apps/cli/src/index.ts` | Replace 49 registrations with ~30 |
| `apps/cli/src/__tests__/commands.test.ts` | Update assertions for new structure |

---

### Task 1: Shared `lib/api.ts` + Fix Output Contract

**Files:**
- Create: `apps/cli/src/lib/api.ts`
- Modify: `apps/cli/src/commands/plan.ts`
- Modify: `apps/cli/src/commands/review.ts`
- Modify: `apps/cli/src/commands/task.ts`

- [ ] **Step 1: Read the existing private helpers for reference**

Read these files to confirm exact patterns:
- `apps/cli/src/commands/plan.ts` (lines 1-27) — `requireServer()` + `fetchApi()`
- `apps/cli/src/commands/review.ts` (lines 1-27) — same
- `apps/cli/src/commands/task.ts` (lines 1-30) — `fetchTaskApi()` (auto-parses JSON, throws on error)
- `apps/cli/src/lib/store.ts` — confirm `getServerUrl` export path
- `apps/cli/src/lib/output.ts` — confirm `outputError`, `output`, `outputQuiet`, `isJson` signatures

- [ ] **Step 2: Create `apps/cli/src/lib/api.ts`**

```typescript
import { outputError } from "./output";
import { getServerUrl } from "./store";

/**
 * Returns the configured server URL or exits with an error.
 */
export function requireServer(): string {
    const serverUrl = getServerUrl();
    if (!serverUrl) {
        outputError('No server URL configured. Run "fubbik init" first.');
        process.exit(1);
    }
    return serverUrl;
}

/**
 * Fetch from the fubbik API. Prepends the server URL + /api prefix.
 * Does NOT check res.ok — use fetchApiJson for that.
 */
export async function fetchApi(path: string, opts?: RequestInit): Promise<Response> {
    const serverUrl = requireServer();
    return fetch(`${serverUrl}/api${path}`, {
        ...opts,
        headers: {
            "Content-Type": "application/json",
            ...opts?.headers,
        },
    });
}

/**
 * Fetch from the API, check status, and parse JSON.
 * Throws an Error with status + body text on non-2xx responses.
 */
export async function fetchApiJson<T = unknown>(path: string, opts?: RequestInit): Promise<T> {
    const res = await fetchApi(path, opts);
    if (!res.ok) {
        const body = await res.text().catch(() => "");
        throw new Error(`API ${res.status}: ${body}`);
    }
    return res.json() as Promise<T>;
}
```

- [ ] **Step 3: Migrate `plan.ts` to use shared helpers**

In `apps/cli/src/commands/plan.ts`:

1. Delete the private `requireServer()` function (lines 9-16 approximately)
2. Delete the private `fetchApi()` function (lines 18-27 approximately)
3. Add import at the top:
   ```typescript
   import { fetchApi } from "../lib/api";
   ```
   
   Note: `plan.ts` uses `fetchApi` (raw Response) not `fetchApiJson`, because it handles errors individually per action. Keep using `fetchApi` — the shared version has the same signature.

4. Also fix the `requireServer` error path — the old private version used `console.error()`. Since we deleted it and now import `fetchApi` (which calls `requireServer` internally), the error path is fixed automatically.

- [ ] **Step 4: Migrate `review.ts` to use shared helpers AND fix output contract**

In `apps/cli/src/commands/review.ts`:

1. Delete the private `requireServer()` function
2. Delete the private `fetchApi()` function
3. Add imports:
   ```typescript
   import { fetchApi } from "../lib/api";
   ```

4. **Fix `listProposals` output contract** — find the action handler. Replace the raw `console.log()` calls with proper `output()` usage:

   The current code uses `console.log(formatDim("No proposals found."))` and loops with `console.log()` for each proposal. Replace the entire human-output section with:

   ```typescript
   // After fetching proposals and checking isJson:
   if (proposals.length === 0) {
       output(cmd, [], formatDim("No proposals found."));
       return;
   }

   const lines: string[] = [];
   for (const p of proposals) {
       const icon = statusIcon(p.status);
       const fields = Object.keys(p.changes).join(", ");
       const age = timeSince(p.createdAt);
       lines.push(
           `  ${icon} ${formatBold(p.chunkTitle ?? p.chunkId.slice(0, 8))} ${formatDim(`[${fields}]`)} ${formatDim(age)} ${formatDim(p.id.slice(0, 8))}`,
       );
       if (p.reason) {
           lines.push(`    ${formatDim(p.reason)}`);
       }
   }
   lines.push(formatDim(`\n${proposals.length} proposal(s)`));
   output(cmd, proposals, lines.join("\n"));
   ```

5. **Fix `showProposal` output contract** — replace all `console.log()` calls with a single `output()`:

   ```typescript
   // Build the human text:
   const lines: string[] = [
       `${formatBold("Proposal")} ${proposal.id}`,
       `${formatDim("Chunk:")} ${proposal.chunkId}`,
       `${formatDim("Status:")} ${proposal.status}`,
       `${formatDim("Proposed by:")} ${proposal.proposedBy}`,
       `${formatDim("Created:")} ${new Date(proposal.createdAt).toLocaleString()}`,
   ];
   if (proposal.reason) lines.push(`${formatDim("Reason:")} ${proposal.reason}`);
   lines.push(`${formatDim("Changes:")}`);
   for (const [field, value] of Object.entries(proposal.changes)) {
       const display =
           typeof value === "string" && value.length > 80
               ? `${value.slice(0, 80)}…`
               : JSON.stringify(value);
       lines.push(`  ${formatBold(field)}: ${display}`);
   }
   output(cmd, proposal, lines.join("\n"));
   ```

6. **Add `outputQuiet` calls** to `approveProposal` and `rejectProposal` — before the existing `output()` call, add:
   ```typescript
   outputQuiet(cmd, proposal.id);
   ```
   
   Make sure `outputQuiet` is imported from `../lib/output`.

- [ ] **Step 5: Migrate `task.ts` to use shared helpers**

Read `apps/cli/src/commands/task.ts` fully. It has `fetchTaskApi` which auto-parses JSON and throws. Replace:

1. Delete the private `fetchTaskApi` function
2. Add import:
   ```typescript
   import { fetchApiJson } from "../lib/api";
   ```
3. Replace all `fetchTaskApi(path, opts)` calls with `fetchApiJson(path, opts)`. The behavior is the same — both parse JSON and throw on error.

- [ ] **Step 6: Check for any other command files with private server helpers**

Run: `grep -rn "requireServer\|getServerUrl\|function fetchApi\|function fetchTask" apps/cli/src/commands/ | head -20`

If any other files still have private helpers, migrate them to use `lib/api.ts` the same way.

- [ ] **Step 7: Type check**

Run: `pnpm --filter cli run check-types 2>&1 | grep -v "gaps\|init\|tag-normalize" | tail -20`

Expected: zero new errors.

- [ ] **Step 8: Commit**

```bash
git add apps/cli/src/lib/api.ts apps/cli/src/commands/plan.ts apps/cli/src/commands/review.ts apps/cli/src/commands/task.ts
git commit -m "refactor(cli): extract shared lib/api.ts, fix review output contract

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

### Task 2: Create Group Command Files + Rename Subcommands

**Files:**
- Create: `apps/cli/src/commands/chunk.ts`
- Create: `apps/cli/src/commands/context-group.ts`
- Create: `apps/cli/src/commands/tag-group.ts`
- Create: `apps/cli/src/commands/req.ts` (rename from `requirements.ts` group)
- Create: `apps/cli/src/commands/maintain.ts`
- Modify: `apps/cli/src/commands/context.ts` (rename command)
- Modify: `apps/cli/src/commands/context-dir.ts` (rename command)
- Modify: `apps/cli/src/commands/context-for.ts` (rename command)
- Modify: `apps/cli/src/commands/tag-normalize.ts` (rename command)
- Modify: `apps/cli/src/commands/import-requirements.ts` (rename command)

- [ ] **Step 1: Create `apps/cli/src/commands/chunk.ts`**

```typescript
import { Command } from "commander";

import { addCommand } from "./add";
import { bulkAddCommand } from "./bulk-add";
import { catCommand } from "./cat";
import { enrichCommand } from "./enrich";
import { getCommand } from "./get";
import { linkCommand } from "./link";
import { listCommand } from "./list";
import { quickCommand } from "./quick";
import { removeCommand } from "./remove";
import { searchCommand } from "./search";
import { unlinkCommand } from "./unlink";
import { updateCommand } from "./update";

export const chunkCommand = new Command("chunk")
    .description("Manage knowledge chunks")
    .addCommand(addCommand)
    .addCommand(bulkAddCommand)
    .addCommand(catCommand)
    .addCommand(enrichCommand)
    .addCommand(getCommand)
    .addCommand(linkCommand)
    .addCommand(listCommand)
    .addCommand(quickCommand)
    .addCommand(removeCommand)
    .addCommand(searchCommand)
    .addCommand(unlinkCommand)
    .addCommand(updateCommand);
```

- [ ] **Step 2: Rename context subcommands and create group**

In `apps/cli/src/commands/context.ts`, find the line like `new Command("context")` and add `.name("export")` to rename it. If the command is created as `const contextCommand = new Command("context")`, change to:

```typescript
// Add .name("export") — this overrides the Commander-registered name
// so when mounted under the context group, it appears as "fubbik context export"
```

Find the exact `new Command(...)` call and change the string argument from `"context"` to `"export"`. Example: if it reads `new Command("context")`, change to `new Command("export")`.

In `apps/cli/src/commands/context-dir.ts`, change the `new Command("context-dir")` to `new Command("dir")`.

In `apps/cli/src/commands/context-for.ts`, change the `new Command("context-for")` to `new Command("for")`.

Create `apps/cli/src/commands/context-group.ts`:

```typescript
import { Command } from "commander";

import { contextCommand } from "./context";
import { contextDirCommand } from "./context-dir";
import { contextForCommand } from "./context-for";

export const contextGroupCommand = new Command("context")
    .description("Export context for AI consumption")
    .addCommand(contextCommand)
    .addCommand(contextDirCommand)
    .addCommand(contextForCommand);
```

- [ ] **Step 3: Rename tag subcommand and create group**

In `apps/cli/src/commands/tag-normalize.ts`, change `new Command("tag-normalize")` to `new Command("normalize")`.

Create `apps/cli/src/commands/tag-group.ts`:

```typescript
import { Command } from "commander";

import { tagsCommand } from "./tags";
import { tagNormalizeCommand } from "./tag-normalize";

export const tagGroupCommand = new Command("tag")
    .description("Manage tags and tag types")
    .addCommand(tagsCommand)
    .addCommand(tagNormalizeCommand);
```

Note: `tagsCommand` from `tags.ts` is a single command (registered as `"tags"`). When mounted under `tag`, it becomes `fubbik tag tags`. That reads oddly. **Check:** read `tags.ts` more carefully. If it registers as `new Command("tags")`, rename it to `new Command("list")` so the user types `fubbik tag list`. If it already has subcommands, mount them directly instead. Adapt based on what you find.

- [ ] **Step 4: Rename import-requirements and create req group**

In `apps/cli/src/commands/import-requirements.ts`, change `new Command("import-requirements")` to `new Command("import")`.

Create `apps/cli/src/commands/req.ts` (NOT overwriting the existing `requirements.ts`):

```typescript
import { Command } from "commander";

import { importRequirementsCommand } from "./import-requirements";
import { requirementsCommand } from "./requirements";

export const reqCommand = new Command("req")
    .description("Manage requirements")
    .addCommand(requirementsCommand)
    .addCommand(importRequirementsCommand);
```

**Check:** `requirementsCommand` already has subcommands (`list`, `add`, `status`, `export`, `verify`). When mounted under `req`, the user types `fubbik req list`, `fubbik req add`, etc. If `requirementsCommand` registers as `new Command("requirements")` with subcommands, we need to hoist its subcommands directly into `reqCommand` instead of nesting. Read the file and decide:

- If `requirementsCommand` has `.addCommand()` children → iterate them and add each to `reqCommand` directly. Don't nest `requirementsCommand` as a sub-sub-group.
- If `requirementsCommand` is a single command → mount it with a rename.

Adapt the code based on what you find.

- [ ] **Step 5: Create `apps/cli/src/commands/maintain.ts`**

```typescript
import { Command } from "commander";

import { cleanupCommand } from "./cleanup";
import { doctorCommand } from "./doctor";
import { healthCommand } from "./health";
import { lintCommand } from "./lint";
import { seedConventionsCommand } from "./seed-conventions";

export const maintainCommand = new Command("maintain")
    .description("Maintenance and diagnostics")
    .addCommand(cleanupCommand)
    .addCommand(doctorCommand)
    .addCommand(healthCommand)
    .addCommand(lintCommand)
    .addCommand(seedConventionsCommand);
```

- [ ] **Step 6: Type check**

Run: `pnpm --filter cli run check-types 2>&1 | grep -v "gaps\|init\|tag-normalize" | tail -20`

Expected: zero new errors. The renamed commands may show warnings about unused imports in index.ts — that's fine, Task 3 rewires it.

- [ ] **Step 7: Commit**

```bash
git add apps/cli/src/commands/chunk.ts apps/cli/src/commands/context-group.ts apps/cli/src/commands/tag-group.ts apps/cli/src/commands/req.ts apps/cli/src/commands/maintain.ts apps/cli/src/commands/context.ts apps/cli/src/commands/context-dir.ts apps/cli/src/commands/context-for.ts apps/cli/src/commands/tag-normalize.ts apps/cli/src/commands/import-requirements.ts apps/cli/src/commands/tags.ts apps/cli/src/commands/requirements.ts
git commit -m "refactor(cli): create command groups for chunk, context, tag, req, maintain

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

### Task 3: Rewire `index.ts` + Update Tests

**Files:**
- Modify: `apps/cli/src/index.ts`
- Modify: `apps/cli/src/__tests__/commands.test.ts`

- [ ] **Step 1: Rewrite `apps/cli/src/index.ts`**

Read the current file first. Then replace the import block and registration block.

**New imports** (replace the ~49 individual imports):

```typescript
import { Command } from "commander";

// Group commands
import { chunkCommand } from "./commands/chunk";
import { codebaseCommand } from "./commands/codebase";
import { contextGroupCommand } from "./commands/context-group";
import { maintainCommand } from "./commands/maintain";
import { planCommand } from "./commands/plan";
import { reqCommand } from "./commands/req";
import { reviewCommand } from "./commands/review";
import { tagGroupCommand } from "./commands/tag-group";

// Remaining top-level commands
import { checkFilesCommand } from "./commands/check-files";
import { diffCommand } from "./commands/diff";
import { docsCommand } from "./commands/docs";
import { exportCommand } from "./commands/export";
import { exportSiteCommand } from "./commands/export-site";
import { gapsCommand } from "./commands/gaps";
import { generateCommand } from "./commands/generate";
import { hooksCommand } from "./commands/hooks";
import { importCommand } from "./commands/import";
import { initCommand } from "./commands/init";
import { kbDiffCommand } from "./commands/kb-diff";
import { mcpToolsCommand } from "./commands/mcp-tools";
import { openCommand } from "./commands/open";
import { promptCommand } from "./commands/prompt";
import { recapCommand } from "./commands/recap";
import { statsCommand } from "./commands/stats";
import { statusCommand } from "./commands/status";
import { suggestCommand } from "./commands/suggest";
import { syncCommand } from "./commands/sync";
import { syncClaudeMdCommand } from "./commands/sync-claude-md";
import { taskCommand } from "./commands/task";
import { watchCommand } from "./commands/watch";
import { whyCommand } from "./commands/why";

import { generateZshCompletions } from "./lib/completions";
```

**New registration block** (replace the ~49 `.addCommand()` calls):

```typescript
// Groups
program.addCommand(chunkCommand);
program.addCommand(codebaseCommand);
program.addCommand(contextGroupCommand);
program.addCommand(maintainCommand);
program.addCommand(planCommand);
program.addCommand(reqCommand);
program.addCommand(reviewCommand);
program.addCommand(tagGroupCommand);

// Top-level
program.addCommand(initCommand);
program.addCommand(statsCommand);
program.addCommand(statusCommand);
program.addCommand(syncCommand);
program.addCommand(syncClaudeMdCommand);
program.addCommand(importCommand);
program.addCommand(exportCommand);
program.addCommand(exportSiteCommand);
program.addCommand(openCommand);
program.addCommand(whyCommand);
program.addCommand(gapsCommand);
program.addCommand(suggestCommand);
program.addCommand(generateCommand);
program.addCommand(recapCommand);
program.addCommand(watchCommand);
program.addCommand(promptCommand);
program.addCommand(diffCommand);
program.addCommand(kbDiffCommand);
program.addCommand(hooksCommand);
program.addCommand(checkFilesCommand);
program.addCommand(docsCommand);
program.addCommand(mcpToolsCommand);
program.addCommand(taskCommand);
```

Keep the `program` declaration, `.name()`, `.description()`, `.version()`, `.option()` lines unchanged. Keep the `completions` subcommand and `program.parse()` unchanged.

**Delete** the old imports that are no longer used (add, get, list, search, update, remove, cat, quick, bulk-add, enrich, link, unlink, context, context-dir, context-for, tags, tag-normalize, requirements, import-requirements, doctor, cleanup, lint, health, seed-conventions).

- [ ] **Step 2: Update `apps/cli/src/__tests__/commands.test.ts`**

Read the current file. The test currently asserts:
- Root help contains: `plan`, `check-files`, `sync-claude-md`, `context-for`, `hooks`, `completions`
- Plan --help shows subcommands

Update the assertions:
- Root help should now contain: `chunk`, `context`, `plan`, `review`, `tag`, `req`, `maintain`, `codebase`, `check-files`, `sync-claude-md`, `hooks`, `completions`
- Root help should NOT contain: `list`, `add`, `search`, `get`, `update`, `remove` (these moved under `chunk`)
- `context-for` is gone from top-level (now `fubbik context for`)
- Add a test that `chunk --help` shows: `add`, `get`, `list`, `search`, `update`, `remove`, `cat`, `quick`, `bulk-add`, `enrich`, `link`, `unlink`
- Plan --help assertions stay the same (plan didn't change)

Example updated test:

```typescript
it("root help contains grouped commands", () => {
    const help = program.helpInformation();
    for (const cmd of ["chunk", "context", "plan", "review", "tag", "req", "maintain", "codebase"]) {
        expect(help).toContain(cmd);
    }
});

it("root help does not contain moved commands", () => {
    const help = program.helpInformation();
    // These are now under 'chunk'
    for (const cmd of [" list ", " add ", " search ", " get ", " update ", " remove "]) {
        expect(help).not.toContain(cmd);
    }
});

it("chunk --help shows subcommands", () => {
    const chunk = program.commands.find(c => c.name() === "chunk");
    expect(chunk).toBeDefined();
    const help = chunk!.helpInformation();
    for (const sub of ["add", "get", "list", "search", "update", "remove", "cat", "quick", "bulk-add", "enrich", "link", "unlink"]) {
        expect(help).toContain(sub);
    }
});
```

Adapt to match the actual test file's style (it may use `describe`/`it` blocks, `expect().toContain()`, etc.).

- [ ] **Step 3: Run the tests**

```bash
pnpm --filter cli test 2>&1 | tail -20
```

Expected: all tests pass. If the test framework uses snapshots, update them.

- [ ] **Step 4: Type check**

```bash
pnpm --filter cli run check-types 2>&1 | grep -v "gaps\|init\|tag-normalize" | tail -20
```

Expected: zero new errors. Old pre-existing errors in `gaps.ts`, `init.ts`, `tag-normalize.ts` are acceptable.

- [ ] **Step 5: Verify the CLI help output**

```bash
pnpm --filter cli run dev -- --help 2>&1 | head -40
```

Expected: ~30 commands listed. `chunk`, `context`, `tag`, `req`, `maintain` visible as groups. `list`, `add`, `search` etc NOT visible at top level.

```bash
pnpm --filter cli run dev -- chunk --help 2>&1
```

Expected: 12 subcommands listed under `chunk`.

```bash
pnpm --filter cli run dev -- context --help 2>&1
```

Expected: `export`, `dir`, `for` subcommands.

- [ ] **Step 6: Commit**

```bash
git add apps/cli/src/index.ts apps/cli/src/__tests__/commands.test.ts
git commit -m "refactor(cli): rewire index.ts to use grouped commands, update tests

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```
