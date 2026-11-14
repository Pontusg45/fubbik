# CLI reference

Reference for CLAUDE.md. Read when adding or changing `fubbik` CLI commands; `fubbik --help` is authoritative.

One Rust binary, `fubbik` (`crates/fubbik`; command implementations in `crates/fubbik-cli`). Run it from source with
`cargo run -q -p fubbik -- <command>`; `fubbik --help` and `fubbik <command> --help` are the full reference. Global flags: `--json`,
`-q/--quiet`, `--url <URL>` (overrides `FUBBIK_URL` and `fubbik.config.json`). Unknown commands are looked up as `fubbik-<name>`
plugin executables (`fubbik plugin list`).

Server and tooling:

- `fubbik serve [--port <n>] [--host <addr>]` — run the API server and web UI (default `127.0.0.1:3000`; `PORT`/`HOST` env)
- `fubbik seed [--scenario minimal|demo|extended] [--only <modules>] [--skip <modules>] [--reset auto|none]` — seed the database
- `fubbik mcp` — run the MCP server over stdio; `fubbik mcp-tools` lists its tools
- `fubbik openapi` — print the OpenAPI document to stdout
- `fubbik backfill-connections` — one-time projection of existing connections into the AGE graph
- `fubbik completions <shell>` — generate shell completions (bash, elvish, fish, powershell, zsh)

Knowledge base:

- `fubbik init` — initialize fubbik configuration in the current project (`--server <url>`, `--force`); `fubbik setup` scans the project
  and populates its knowledge base; `fubbik doctor` diagnoses configuration and connectivity; `fubbik config show/set`
- `fubbik add/get/cat/update/delete/list/search` — chunk CRUD (colored output, table formatting); the same verbs exist under
  `fubbik chunk …` (`remove` there instead of `delete`)
- `fubbik add <title>` takes `-c/--content`, `-f/--file`, `--stdin`, `-t/--type`, `--tags`, `--space`
- `fubbik quick "title"` — one-liner chunk creation (supports `--type`, `--tags`, `--space`, `--global`, pipe stdin for content; no space
  unless `--space` is given)
- `fubbik link <source> <target> [-r <relation>]` / `fubbik unlink <id>` — manage connections
- `fubbik tag list/add/rename/remove/normalize` — tag management (`tags` is an alias)
- `fubbik space add/list/remove/current` — space management (`codebase` is an alias)
- `fubbik export [--format json|md]` / `fubbik import <path>` — bulk operations (import reads JSON or Markdown files or directories);
  `fubbik bulk-add` imports JSONL; `fubbik export-site` generates a static HTML site
- `fubbik docs extract/list/show/import/import-dir/sync/render` — imported Markdown documents. `docs import` takes a file or a
  directory; a directory is walked recursively and imported one file at a time, failures are listed on stderr, and the command exits
  non-zero if any file failed. `import-dir` shares the walk but sends every file in one batch request, and fails only if that request
  fails. Both skip `node_modules`, `target`, `.git`, `dist` and `legacy`, and name the skipped directories on stderr
- `fubbik enrich [<id>] [--all]` — AI enrichment
- `fubbik health/stats/status` — system info
- `fubbik stale list/dismiss`, `fubbik lint`, `fubbik cleanup`, `fubbik gaps`, `fubbik recap`, `fubbik kb-diff`, `fubbik updates` —
  knowledge maintenance
- `fubbik review list/show/approve/reject` — proposed knowledge changes
- `fubbik prompt list/get/add` — reusable prompt templates
- `fubbik open` — open the web app, a named page, or a chunk

Plans, tasks, requirements, matrices:

- `fubbik plan create <title>` — create a plan
- `fubbik plan list` — list plans
- `fubbik plan show <id>` — show plan detail
- `fubbik plan status <id> <status>` — update plan status
- `fubbik plan add-task <planId> <title>` — add a task
- `fubbik plan task-done <planId> <taskId>` — mark a task done
- `fubbik plan link-requirement <planId> <requirementId>` — link a requirement
- `fubbik task add/list/claim/done` — quick tasks backed by single-task plans
- `fubbik req list/add/status/export/verify/import` — requirements (`requirements` is an alias)
- `fubbik matrix list/create/show/add-dimension/add-rule/cell/gaps/link/link-code/report-test/history/behaviors-for` — behavioral matrices

Files and context:

- `fubbik check-files [files...] [--staged]` — check files against chunk knowledge
- `fubbik hooks install/uninstall` — git pre-commit hook management
- `fubbik why <file>`, `fubbik suggest <file>`, `fubbik watch` — knowledge associated with files
- `fubbik context export` — token-budgeted context export (with `--for <path>` file relevance boost)
- `fubbik context for <path>` — generate context document for a file
- `fubbik context dir <dir>` — generate CLAUDE.md-style context for a directory
- `fubbik context about/for-plan/for-diff/snapshot/claude-md` — concept, plan and diff context, snapshots, CLAUDE.md output
- `fubbik sync [--watch]` — regenerate the configured CLAUDE.md from tagged chunks (`sync-claude-md` is an alias)
- `fubbik generate claude.md/agents.md/cursorrules` — generate instruction files; `fubbik seed-conventions` extracts conventions from one
- `add`, `quick`, `import`, `export`, `sync` and the `context` subcommands (`context snapshot create` for snapshots) take
  `--space <name>` (`--codebase` is an alias); `quick` and `sync` also take `--global`

