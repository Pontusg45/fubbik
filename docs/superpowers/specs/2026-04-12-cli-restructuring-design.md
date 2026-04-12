# CLI Restructuring

## Problem

The fubbik CLI has 49 top-level commands. Running `fubbik --help` produces a wall of text where related commands (`add`, `get`, `list`, `search`, `update`, `remove`, `cat`, `quick`, `bulk-add`) sit as peers with unrelated utilities (`doctor`, `watch`, `why`). Discoverability is poor.

Additionally:
- `plan.ts` and `review.ts` each have private `requireServer()` / `fetchApi()` helpers that bypass the shared output contract (`output()`, `outputError()`, `outputQuiet()`)
- `review.ts` uses raw `console.log()` instead of the output helpers, breaking `--json` and `--quiet` support
- Several command files duplicate the same error-handling boilerplate (`if (!res.ok) { outputError(...); process.exit(1); }`)

## Goal

1. Extract a shared `lib/api.ts` module with `requireServer`, `fetchApi`, and `fetchApiJson` helpers. Fix output contract violations.
2. Group related commands into 6 namespaces, reducing top-level commands from 49 to ~30 (22 top-level + 8 groups).
3. Hard break — old top-level names (e.g., `fubbik list`) stop working. No aliases.

---

## 1. Shared `lib/api.ts`

### New file: `apps/cli/src/lib/api.ts`

Exports:

- **`requireServer(): string`** — reads server URL from the store via `getServerUrl()`. If not configured, calls `outputError('No server URL configured. Run "fubbik init" first.')` and exits. Returns the URL string.

- **`fetchApi(path: string, opts?: RequestInit): Promise<Response>`** — prepends `${serverUrl}/api` to the path, sets `Content-Type: application/json` header, forwards `opts`. Does NOT check `res.ok` — leaves that to the caller or `fetchApiJson`.

- **`fetchApiJson<T>(path: string, opts?: RequestInit): Promise<T>`** — calls `fetchApi`, checks `res.ok`. If the response is not OK, reads the body text and throws an error with the status + body. If OK, returns `res.json() as T`. This eliminates the repetitive 4-line error-check block from every command action.

### Migration

Every command file that currently has a private `requireServer` / `fetchApi` / inline server-calling code switches to importing from `lib/api.ts`:

- `plan.ts` (lines 9-27) — delete private helpers, import from `lib/api`
- `review.ts` (lines 9-27) — delete private helpers, import from `lib/api`
- `task.ts` — if it has its own fetch helper, migrate
- `health.ts`, `status.ts` — if they have inline fetch, migrate
- Any other command file with `getServerUrl()` + `fetch()` calls

### Output contract fix

In `review.ts`:
- `listProposals`: replace all raw `console.log()` calls with `output(cmd, data, formattedString)` for human mode, and ensure `isJson(cmd)` branch emits JSON
- `showProposal`: same — use `output()` instead of raw `console.log()`
- `approveProposal` and `rejectProposal`: add `outputQuiet(cmd, proposal.id)` before the `output()` call, so `--quiet` mode emits just the ID (composable)

In `plan.ts`:
- Replace the private `requireServer` error path (`console.error(...)`) with `outputError(...)`

---

## 2. Command Grouping

Six new group command files. Each composes existing subcommand files without rewriting their internals.

### `fubbik chunk` — `apps/cli/src/commands/chunk.ts`

Composes: `add`, `get`, `list`, `search`, `update`, `remove`, `cat`, `quick`, `bulk-add`, `enrich`, `link`, `unlink`

Each existing command file already exports a `Command` instance. The group file imports them and calls `.addCommand()`:

```typescript
export const chunkCommand = new Command("chunk")
    .description("Manage knowledge chunks")
    .addCommand(addCommand)
    .addCommand(getCommand)
    // ...
```

If any subcommand's registered name clashes with its new role under the group (e.g., `listCommand` registers as `list` which is fine), no rename needed. If a command registers with a name that doesn't make sense under the group, add a `.name("newname")` call.

### `fubbik context` — `apps/cli/src/commands/context-group.ts`

Composes:
- `export` — from current `context.ts` (rename the command from `context` to `export` via `.name("export")`)
- `dir` — from `context-dir.ts` (rename from `context-dir` to `dir`)
- `for` — from `context-for.ts` (rename from `context-for` to `for`)

### `fubbik tag` — `apps/cli/src/commands/tag-group.ts`

Composes:
- The existing `tags.ts` command (which may itself have subcommands). Rename from `tags` to keep sub-structure, or flatten — check the actual file.
- `normalize` — from `tag-normalize.ts` (rename from `tag-normalize` to `normalize`)

### `fubbik req` — `apps/cli/src/commands/req.ts`

Composes:
- The existing `requirements.ts` command (rename from `requirements` if needed to avoid stuttering like `fubbik req requirements`)
- `import` — from `import-requirements.ts` (rename from `import-requirements` to `import`)

### `fubbik maintain` — `apps/cli/src/commands/maintain.ts`

Composes: `doctor`, `cleanup`, `lint`, `health`, `seed-conventions`

### Already grouped (no changes)

- `fubbik plan` — `commands/plan.ts` (already a group)
- `fubbik review` — `commands/review.ts` (already a group)
- `fubbik codebase` — `commands/codebase.ts` (already a group)

---

## 3. Rewired `index.ts`

### Grouped commands (8):

```
fubbik chunk [add|get|list|search|update|remove|cat|quick|bulk-add|enrich|link|unlink]
fubbik context [export|dir|for]
fubbik plan [create|list|show|status|add-task|task-done|link-requirement]
fubbik review [list|show|approve|reject|count|propose]
fubbik tag [list|create|...|normalize]
fubbik req [list|create|...|import]
fubbik maintain [doctor|cleanup|lint|health|seed-conventions]
fubbik codebase [add|list|remove|current]
```

### Remaining top-level (~22):

```
init, stats, sync, sync-claude-md, import, export, export-site,
open, why, gaps, suggest, generate, recap, watch, prompt,
diff, kb-diff, hooks, check-files, docs, mcp-tools, task
```

### Shell completions

`lib/completions.ts` generates zsh completions. If it reads the command tree dynamically (via Commander's introspection), it updates automatically. If hardcoded, regenerate after the restructure.

### CLI tests

`apps/cli/src/__tests__/commands.test.ts` asserts subcommand names on the program tree. Update it to reflect the new grouped structure.

---

## 4. Files Changed

### New files

| Path | Responsibility |
|---|---|
| `apps/cli/src/lib/api.ts` | Shared `requireServer`, `fetchApi`, `fetchApiJson` |
| `apps/cli/src/commands/chunk.ts` | Group: chunk subcommands |
| `apps/cli/src/commands/context-group.ts` | Group: context subcommands |
| `apps/cli/src/commands/tag-group.ts` | Group: tag subcommands |
| `apps/cli/src/commands/req.ts` | Group: requirement subcommands |
| `apps/cli/src/commands/maintain.ts` | Group: maintenance subcommands |

### Modified

| Path | Change |
|---|---|
| `apps/cli/src/index.ts` | Replace 49 imports/registrations with ~30 |
| `apps/cli/src/commands/review.ts` | Use shared `lib/api`, fix output contract |
| `apps/cli/src/commands/plan.ts` | Use shared `lib/api`, fix outputError |
| `apps/cli/src/commands/context.ts` | Rename command to `export` |
| `apps/cli/src/commands/context-dir.ts` | Rename command to `dir` |
| `apps/cli/src/commands/context-for.ts` | Rename command to `for` |
| `apps/cli/src/commands/tag-normalize.ts` | Rename command to `normalize` |
| `apps/cli/src/commands/import-requirements.ts` | Rename command to `import` |
| `apps/cli/src/commands/task.ts` | Use shared `lib/api` if applicable |
| Any other command with private `fetchApi` | Use shared `lib/api` |
| `apps/cli/src/__tests__/commands.test.ts` | Update assertions for new structure |

### Unchanged

- Individual command file logic (add.ts, get.ts, list.ts, etc.) — internal behavior stays the same. Only imports and `.name()` calls change.
- `lib/output.ts`, `lib/colors.ts`, `lib/store.ts` — unchanged
- Backend, web, MCP — untouched

---

## 5. Out of Scope

- Adding `--server` flag to `get`/`update`/`remove`
- Fixing `--codebase` flag inconsistency (name vs ID)
- Confirmation prompts on destructive actions
- Renaming `outputQuiet` semantics
- Rewriting command internals or business logic
- Adding new tests beyond updating existing ones
- Interactive mode improvements

---

## Success Criteria

- `fubbik --help` shows ~30 entries instead of 49
- `fubbik chunk --help` shows all 12 chunk subcommands
- `fubbik context --help` shows `export`, `dir`, `for`
- `fubbik tag --help` shows tag management + `normalize`
- `fubbik req --help` shows requirement commands + `import`
- `fubbik maintain --help` shows `doctor`, `cleanup`, `lint`, `health`, `seed-conventions`
- Old top-level names (`fubbik list`, `fubbik add`, `fubbik search`, etc.) are gone — "unknown command" error
- `fubbik review list --json` outputs JSON (output contract fixed)
- `fubbik review approve <id> --quiet` outputs just the proposal ID
- No command file has a private `requireServer` or `fetchApi` — all use `lib/api.ts`
- `pnpm --filter cli run check-types` passes (pre-existing errors in unrelated files acceptable)
- CLI tests in `commands.test.ts` updated and passing
