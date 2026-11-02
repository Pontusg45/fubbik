# fubbik desktop

A native project manager built on [GPUI](https://github.com/zed-industries/zed/tree/main/crates/gpui). Register repos, and see and
control their processes, databases, git state and fubbik knowledge.

```bash
just start-desktop                                          # from the repo root
cargo run --manifest-path apps/desktop/Cargo.toml         # same thing
cargo run --manifest-path apps/desktop/Cargo.toml -- --probe <repo>   # print one status poll, no UI
```

`just desktop-install` (`scripts/install-app.sh`) builds a release binary, wraps it in `~/Applications/fubbik.app` (icon drawn
by `scripts/make-icon.swift`, ad-hoc signed) and pins it to the Dock once (`--no-dock` skips that; `FUBBIK_APP_DIR` changes
the folder). Re-run it after changes; the Dock entry is kept.

## Built-in server

Starting the app is all fubbik needs. Docker, `fubbik serve` and `just dev` aren't needed.

- **The API runs inside the app**, on `127.0.0.1:3000` (`FUBBIK_DESKTOP_PORT` changes the port). The CLI, MCP, VS Code and
  Claude Code reach it as usual while the app is open, and the web UI is served there too (from `apps/web/dist`, bundled by
  `just desktop-install`). The app runs the same `fubbik_api::server::serve` as `fubbik serve`, with every feature on and the
  local dev session.
- **The CLI and MCP find it through `~/.config/fubbik/server.json`** (`$XDG_CONFIG_HOME/fubbik`), which holds the URL and the
  server's pid. It is written while the server runs, and ignored once that pid is gone. An explicit `--url`, `FUBBIK_URL` or
  `serverUrl` still wins. When nothing answers, the error asks whether the desktop app or `fubbik serve` is running.
- **Its own PostgreSQL 18**, a private cluster in `~/Library/Application Support/fubbik/pgdata` (`FUBBIK_DESKTOP_DATA` moves
  it). It listens only on a Unix socket in `…/fubbik/run`, or a private `/tmp/fubbik-<uid>` when that path would pass the
  103-byte socket limit. It starts with the app (JIT off) and stops when the app quits (fast shutdown).
  - **First run:** `initdb` (ICU collation), the migrations, and the `core` seed (built-in types, relations, templates).
  - **Binaries:** `fubbik.app/Contents/Resources/postgres`, a trimmed copy of Postgres.app 18 (about 130 MB). It has the
    server tools, `pg_dump`/`pg_restore`, pgvector and `pg_trgm`, but not PostGIS or the language handlers. Otherwise
    `FUBBIK_POSTGRES_BIN`, Postgres.app, then Homebrew `postgresql@18`; each needs pgvector.
  - **AGE:** `install-app.sh` also builds Apache AGE once (`scripts/build-age.sh`: the pinned `PG18/v1.7.0-rc0`, against the
    bundled Postgres, cached in `~/Library/Caches/fubbik`) and bundles it, so graph projection works. It needs Bison 3
    (`brew install bison`). If the build fails, the app runs without AGE, as on any Postgres without it.
    `FUBBIK_SKIP_AGE=1` skips the build.
  - **Major versions:** the data folder only opens with the PostgreSQL major that created it. A folder from another version is
    refused with what to do: back it up with that version's `pg_dump`, move the folder aside, restart, then import the dump
    (or run `pg_upgrade`).
- **Status:** the sidebar's footer shows it (running, starting, using another server, or failed; click it for Setup). A
  failure raises a toast. Logs → "built-in server" is the API's log (`backend.log`, `RUST_LOG` filters it), and "built-in
  postgres" is `postgres.log`.
- **Already running?** When something healthy answers on the port (for example `just dev`), the app uses it and starts nothing.
  `just kill-dev` refuses to kill the app's server.
- **Existing data:** Setup → "Import existing data" lists the default `postgresql://localhost:5432/fubbik` and the PostgreSQL
  databases in your repos' services files.
  - Import runs live: the API pauses, the built-in database is replaced by a copy (the source is only read, with `pg_dump`),
    and the API comes back.
  - A toast says how many objects were skipped (AGE's graph, when the bundle has no AGE). `import.log` has `pg_restore`'s
    messages.
- **Backups:** one a day while the server runs, the last 7 kept, in `…/fubbik/backups` (`pg_dump -Fc`; restore one with
  `pg_restore`). Setup has "Back up now" and "Show in Finder".
- **Quitting** stops the server. "Quit, Keep Server Running" (the fubbik menu, or Setup) hands it to a background copy of the
  app (`fubbik-desktop --serve`, no window), so MCP and the CLI keep working. The next launch uses that server, and Setup →
  "Stop background server" ends it.
- **Turning it off:** `backend = "off"` in `desktop.toml` (or the Setup toggle) makes the app start nothing. Run `fubbik serve`
  yourself then.
- **Files:** `auth-secret` in the data folder signs session cookies (generated once).

## Home

The app opens at Home (⌘0, or "Home" at the top of the sidebar). Home has four tabs: **Projects**, every registered repo
as a card with the computer beside them; **All agents** (⇧⌘A), every repo's live agents and recent sessions; and **Tools**
(⇧⌘T, `src/tools.rs`):

- **Services:** `gh` signed in, Docker / OrbStack, Ollama and the app's tmux server, each with a fix when it is off (sign in,
  start OrbStack, start Ollama).
- **Toolchain:** git, gh, tmux, just, node, pnpm, mise, bun, cargo, rustc, zig, docker, psql, ollama, claude, codex and
  libgit2 — version, path, what it is for, "missing", the newer version Homebrew offers (`brew outdated`), and "upgraded
  today / yesterday" (a silent upgrade is a usual cause of "it worked yesterday"). Homebrew facts apply only to Homebrew's
  copy (a node from mise is not Homebrew's). Install, Upgrade, "Upgrade all" (asks first) and "brew update" run in a
  terminal tab.
- **Utilities:** copy a new UUID, the Unix time or the UTC time, and open the app's config folder.
- **Local domains** (`src/proxy.rs`): every process with a port can get `<process>.<repo>.test`, served by Caddy with
  `tls internal` and `reverse_proxy localhost:<port>`. "Add domain" only marks it (desktop.toml `domains`); "Update
  Caddyfile…" (asks first) writes the chosen sites into one marked block of `~/.config/caddy/Caddyfile`, backed up to
  `Caddyfile.fubbik-backup`, leaving your own sites alone. "Add hosts entries…" runs `sudo tee -a /etc/hosts` in a
  terminal tab (the password prompt shows there); "Start Caddy" runs `caddy run --watch` in a tab, so later writes reload
  it. Each row says what is still missing (Caddyfile, hosts entry, Caddy running). Ports declared by two processes, in
  one repo or across repos, are flagged red: those cannot run at once.
- **Disk** (`src/disk.rs`): each repo's `target/`, `node_modules` (root and workspace packages) and `.turbo`, measured
  with `du`, and Docker's `system df`. Remove (asks first; `cargo clean` for target/, with how to get it back) and
  "Prune Docker…" (`docker system prune -f` in a tab: running containers, volumes and tagged images stay).

**Dependencies** (⇧⌘L, `src/deps.rs`) compares what every registered repo declares: package.json (dependencies, dev, peer;
pnpm `catalog:` resolved from pnpm-workspace.yaml), Cargo.toml (including `[workspace.dependencies]`), and toolchain pins
(`packageManager`, `engines.node`, `.nvmrc`, `.node-version`, `.tool-versions`, `mise.toml`, `rust-toolchain`, Cargo
`rust-version` and `edition`). One row per dependency used by two or more repos, one column per repo, each cell colored by
how far that repo is behind the newest version any repo uses (red: a major behind, 0.x minors count as majors; yellow: a
minor behind). Filters: different majors, any difference, all shared; toolchain, npm, cargo. A row opens to the manifests
that declare it, each opening in the editor, and "Align to <newest> with agents": after a confirmation, each repo that is
behind gets a worktree on `deps/<name>-<version>` and a Claude session told to bump it, fix what breaks, run the checks and
commit there (never push or merge). Declared requirements, not lockfiles; workspace, path and git dependencies are
left out.

- **A card per repo:** status, branch, changes, ahead/behind and the last commit, processes up (one dot each), live and waiting
  agents, worktrees, how many things need you, overlapping worktrees, knowledge chunks and failing requirements. Click a card,
  or press j/k and ⏎, to open the repo.
- **The computer**, sampled every 2 s (`src/system.rs`, using `sysinfo`):
  - **CPU and memory:** CPU use with a two-minute graph, per-core bars and the load average; memory and swap.
  - **Disk:** free space on the data volume, red under 10 GB.
  - **Power and network:** battery (`pmset`), network rates and uptime.
  - **Processes:** the busiest processes by CPU and by memory.
  - **Ports:** every listening TCP port with its process (`lsof`), named after the repo process when one owns it.
- A repo tab (⌘1–⌘9…) or a repo in the sidebar leaves Home. `FUBBIK_DESKTOP_TAB` opens on a tab instead.

## Where processes and scripts run

Starting a process ("Start", "Start all", a profile, the palette, the list keys) or a script asks where it should run:

1. **In the background:** the app runs it, with output in Logs. It stops when the app quits.
2. **In a tmux session:** a detached session on the app's tmux server, named `fk-run-<repo>-<name>`. Attach from any terminal with
   `tmux -L fubbik attach -t fk-run-fubbik-api` (detach with ⌃B D), or from the Processes row's "Terminal" button. It keeps
   running when the app quits, and the app finds it again on the next start.
   - Stop and Restart act on the session (the process group gets SIGTERM, then SIGKILL after 2 s), and Restart reuses its name.
   - Output is also piped to `…/fubbik/runs/<session>.log` for the Logs tab, and the pane stays after the command ends, so
     the exit code stays readable.
3. **In a terminal tab:** a tab in the Terminal view, where you can type into it.
4. **In Terminal.app.**

The prompt's "Always use this" (A) saves the choice as `run_in` in `desktop.toml`. Setup → "Start processes and scripts" changes
it, or switches back to asking. Keys in the prompt: 1–4 choose, A toggles "always", esc cancels.

## MCP server

Other agents can read what the app shows, and start and stop its processes and scripts, through its own MCP server:

- **HTTP:** `http://127.0.0.1:3100/mcp` while the app runs (MCP streamable HTTP with JSON replies; `FUBBIK_DESKTOP_MCP_PORT`
  moves it). Requests with a non-local `Origin` are refused. Claude Code: `claude mcp add --transport http fubbik-desktop
  http://127.0.0.1:3100/mcp`, or the `fubbik-desktop` entry in `agents/mcp/claude-code.json`.
- **stdio:** `fubbik-desktop --mcp` (in the bundle: `~/Applications/fubbik.app/Contents/MacOS/fubbik-desktop --mcp`) forwards to
  the running app, and says so when the app is not running.
- **Tools:**
  - `list_repos`: each repo's status, branch, changes, processes up, live sessions.
  - `get_repo`: git, processes, databases, worktrees, AI sessions, overlaps, scripts, and the knowledge summary.
  - `get_today`: the Today list.
  - `list_sessions`: AI sessions, filtered by repo or state.
  - `get_process_logs`: output of a process or script started from the app.
  - `get_builtin_server`: the built-in server's status and paths.
  - `get_system_stats`: the computer's CPU, memory, disk, battery, network, busiest processes and listening ports (as on Home).
  - `start_process` and `run_script`: `where` is `tmux` (the default, attachable from any terminal), `background` or
    `terminal`. The result names the tmux session and the attach command.
  - `stop_process` and `restart_process`: only for what the app started; anything started elsewhere is left alone.
  - For an agent that keeps track of every project (`src/mcp_more.rs`, read-only):
    - `get_changes_since(cursor?, repo?)`: the change feed (`src/mcp_events.rs`) — commits and branch switches, processes
      up or down, agents started / waiting / ended, worktrees added or removed, PRs opened / merged / needing you, bells,
      dependencies that start or stop disagreeing by a major (checked every 10 min) and tools Homebrew changed (every 30).
      Pass back the returned cursor to get only what is new; cursors survive app restarts (sequence numbers start at the
      start time). The feed keeps 2000 events in memory.
    - `get_dependencies(filter?, ecosystem?, name?)`: the Dependencies tab, with each declaring file.
    - `get_toolchain(refresh?)`: the Tools tab (cached 2 min).
    - `get_pull_requests(repo?, open_only?)`: PRs and checks per branch, plus `errors` for repos gh could not read.
    - `list_worktrees(repo)`: with merged state, sessions, overlaps and the branch's PR.
    - `get_changes(repo, worktree?)` and `get_diff(repo, worktree?, path, staged?)`: a checkout's changed files and one
      file's diff (only the repo's own checkouts, paths inside them).
  - Notes and watches (`src/tracker.rs`, kept in `~/.config/fubbik/tracker.json`): `add_note` / `list_notes` /
    `complete_note` (open todos show on Today under "Next up"), and `add_watch` / `list_watches` / `remove_watch`:
    `event` (a feed event by kind and/or phrase), `process_down` (N minutes) or `agent_waiting` (N minutes). The app
    checks watches while it runs; one that fires raises an alert (`list_alerts`, `dismiss_alert`): a "Needs you" line
    on Today, a toast, and a `watch_fired` feed event. One alert per episode.
  - The feed is also kept in `~/.config/fubbik/events.jsonl` (the newest 2000, read back at start).
- **Testing a second copy:** set `FUBBIK_DESKTOP_TMUX_SERVER` (with `XDG_CONFIG_HOME`, `FUBBIK_DESKTOP_DATA`, the two
  ports) so it gets its own tmux server, hooks, events and tracker files and never touches the running app's.
- **A tracker workspace:** Setup → "Projects tracker" (or ⌘K "Set up the projects tracker") writes, in the repos' common
  folder (`~/projects`), a `.mcp.json` with both servers (`fubbik-desktop` over HTTP and `fubbik mcp`), merged into an
  existing one, and a `CLAUDE.md` with the tracker's job (each check: the change feed from a saved cursor, then Today;
  weekly: dependencies, toolchain, worktrees; read-only, never commits or pushes). An existing CLAUDE.md is kept. Run
  `claude` in that folder and ask "what changed?".
- **How it works:** the UI publishes a JSON view after every poll (`src/ui/mcp_publish.rs`), and `src/mcp.rs` answers from it,
  so calls never wait on git or the network. For the knowledge base itself, use `fubbik mcp`.

It is its own cargo workspace (not a member of the root one under `crates/`), so `cargo build --workspace` and CI never compile GPUI.
It links `fubbik-api` and `fubbik-db` by path for the built-in server; their sqlx queries build from the checked-in `.sqlx/`
(`SQLX_OFFLINE`, set in `apps/desktop/.cargo/config.toml`, the `start-desktop` recipe and `install-app.sh`).
GPUI's `runtime_shaders` feature is on, so builds do not need Xcode's Metal Toolchain download. Developed on macOS; Linux needs GPUI's
X11/Wayland system libraries, and the notifications and "open in Terminal" use macOS tools.

## Files it reads

- **App config:** `~/.config/fubbik/desktop.toml` (or `FUBBIK_DESKTOP_CONFIG`). It holds the registered repos (`[[repo]]` with `path`
  and an optional `group`), and optional `fubbik_url` (else `FUBBIK_URL`, else `http://localhost:3000`), `web_url` (else
  `FUBBIK_WEB_URL`, else `http://localhost:3001`), `fubbik_bin`, `theme`, `terminal`, `terminal_persistence`, `ghostty_theme`, `max_agents` and `global_hotkey`.
- **Per repo:** `fubbik.services.toml` at the repo root lists what to watch. "Generate from repo" writes a starter from `package.json`
  dev scripts and compose database services. This repo's own copy is a worked example.

```toml
[[process]]
name = "rust-tests"
command = "just rust-test-fast"
[process.report]                   # optional: report pass/fail when it exits
requirement = "<requirement id>"   # set passing / failing
matrix = "<matrix id>"             # and record a test result on a cell
cell = "<cell id>"

[[process]]
name = "api"
command = "just start-api"       # run through your login shell, in the repo root (or `cwd`)
port = 3000                        # optional: Running/External is decided by who holds it
health_url = "http://127.0.0.1:3000/api/health"   # optional: failing → Degraded

[[database]]
name = "postgres"
url = "postgresql://localhost:5432/fubbik"
compose_service = "db"             # optional: status, plus default start/stop
start = "…"                        # optional, overrides the compose default; also: stop, migrate, seed, studio
```

## What it does

- **Tests** (⇧⌘U, `src/test_runs.rs`): runs `cargo nextest` (libtest JSON), `cargo test` or vitest (JSON reporter) and
  lists failing tests with their message; Open jumps to the failing file and line in the editor, "Rerun failed" reruns only
  those, and "Send to agent" hands them to a Claude session. Failing tests show on Today, and runs go to the change feed
  (`tests_failed`, `tests_passed`).
- **Editor:** Preferences → Editor picks Zed, Cursor or VS Code (installed ones are listed); unset, `$VISUAL`/`$EDITOR`.
  Files open at a line (`zed file:12`, `cursor -g file:12`); the Changes diff has "Open in editor" at its first changed
  line.
- **Containers** (Databases tab, `src/containers.rs`): the Docker / OrbStack containers that belong to the repo (its
  compose project's folder, or a name or label starting with the repo's name), with state, ports and Logs, Start,
  Restart and Stop (asks first).
- **Database details** (`src/pg.rs`): version, size and connections for each `[[database]]` URL; sqlx migration drift
  (files in the repo not applied, and applied versions with no file); psql in a tab; Snapshot (`pg_dump -Fc` into
  `~/Library/Application Support/fubbik/snapshots`) and "Restore latest" (asks first; `pg_restore --clean`).
- **Claude Code hooks** (Setup, `src/claude_hooks.rs`): "Add hooks…" (asks first) adds SessionStart, UserPromptSubmit,
  Stop, Notification and SessionEnd hooks to `~/.claude/settings.json`, next to yours and backed up first. Each pipes the
  event to `POST /hook` on the MCP port with a 1-second curl that never fails, so agent state (started, working,
  waiting for you, ended) shows at once instead of on the next poll, and lands in the change feed. "Remove hooks"
  takes out exactly those.
- **.env drift** (Setup, `src/env_drift.rs`; off until turned on): each `.env.example` (or `.env.sample`,
  `.env.template`, at the root and two levels down) against the `.env` beside it: keys missing from `.env` and keys the
  example does not list. Key names only: each line is cut at `=` as it is read; values are never kept, shown or sent.
- **Menu bar item** (`src/menu_bar.rs`): `fk`, or `fk 3` when three things need you; its menu lists them (each runs
  Today's action for that line), then Open Today, All agents and Capture note. "Hide the menu bar item" in the command
  palette turns it off (desktop.toml `menu_bar = false`).
- **Raycast:** `integrations/raycast` has Today, Agents, Repos and Capture Note commands over the MCP server (see its
  README).

- **Status** every 3 s. Processes are Running if this app started them, External if someone else holds the port (with that PID and
  a Kill button), Degraded if the health URL fails, otherwise Stopped. Databases are checked through their compose service, or a TCP
  connection to their URL. Git shows branch, ahead/behind, changed files and the last commit. CPU and memory come from `ps`.
- **Control:** start, stop and restart, per process, per repo or per group. Each process runs in its own process group, and stopping
  takes the whole group down (TERM, then KILL after 2 s). Quitting the app stops everything it started. Database actions (start, stop,
  migrate, seed, studio) run as logged tasks.
- **Logs:** the last 2 000 lines for each process or task the app started, with filter, pause and clear.
- **Knowledge:** the fubbik space matched to the repo (by git remote or path): chunk, stale, orphan and thin counts, plans with task
  progress, requirement results, active agents, and a quick-note form. Checked every 30 s. If the API is unreachable, only this tab
  says so.
- **Worktrees:** every `git worktree` of the repo with its branch, dirty count, commits ahead / behind the main branch, and
  main / merged / locked / prunable / missing badges, plus the AI sessions running in it. Open it in the editor or Terminal, start a
  Claude session in it, create one (`git worktree add -b <branch> <parent>/<repo>-<branch>`), remove one (plain
  `git worktree remove`, so git refuses a dirty tree; the branch is kept; refused while an AI session is live in it), or prune
  stale entries. Re-read every 30 s and after each change.
- **AI sessions:** Claude Code and Codex sessions whose working directory is in the repo or any of its worktrees. Live Claude Code
  sessions come from `~/.claude/sessions/<pid>.json` (Working / Waiting); history from `~/.claude/projects/*/<id>.jsonl` (titled by
  its `ai-title` / `custom-title`). Codex sessions come from `~/.codex/sessions/**/rollout-*.jsonl` (Active if written in the
  last 2 minutes), titled from `~/.codex/session_index.jsonl` or the first real prompt. Only metadata lines are parsed, and
  transcripts are read incrementally. Resume opens Terminal with `claude --resume <id>` / `codex resume <id>`.
- **Plans** (⌘7): open plans and their tasks, with who holds each task on the coordination board. "Start in new worktree" or
  "Start here" creates a worktree `task/<slug>` (optional), joins the board with the new session's id as `externalKey`, claims
  the task, moves it to `in_progress`, and opens Terminal with `claude --session-id <id>` and a first prompt built from the
  task, its acceptance criteria and `/api/context/for-plan`. Sessions and worktrees then show the task they hold. With
  `coordination` off, the session still starts, without a claim.
- **Knowledge** (⌘8): space counts and links into the web app; an inbox of stale flags (Open / Dismiss) and pending proposals
  (Approve / Reject; the API lists them across all spaces); enrichment (not fully enriched and stale-embedding counts, Ollama
  state, Enrich all); a gaps scan like `fubbik gaps` (one request per file, first 400); failing and untested requirements
  (Mark passing / failing) and behaviour matrices with violated cells; feature overlays (turn on / off); a quick note. The
  sidebar shows "✕ N failing checks" for failing requirements plus violated cells.
- **Setup** (⌘9), a fuller `fubbik doctor`: server health and enabled groups, the Rust `fubbik` binary (configured
  `fubbik_bin`, a build in a registered fubbik checkout, `~/.cargo/bin`, then `PATH` — only if `--version` prints
  `fubbik <version>`, since the legacy TypeScript CLI has no `mcp`; "Build fubbik" runs `cargo build --release -p fubbik`),
  the fubbik MCP server in `.mcp.json` or `~/.claude.json` ("Add to .mcp.json"), and for each worktree the pre-commit hook
  (the same script as `fubbik hooks install`; another hook is left alone) and the generated CLAUDE.md (in sync / stale / not
  generated against `/api/chunks/export/claude-md`, written verbatim like `fubbik sync`; needs `claudeMd` in
  `fubbik.config.json`).
- **Branch context:** "Context" on a worktree lists the chunks its changes touch: `for-files` context for the files changed
  against main (committed and uncommitted), and the `check-files` view of chunks linked to a changed file by file-ref or
  applies-to.
- **Test results:** a process with a `[process.report]` table (`requirement`, `matrix` + `cell`, `test_ref`) reports its
  outcome when it exits on its own: exit 0 sets the requirement passing and records a passing test on the cell, anything
  else failing.
- **Search and context anywhere:** in ⌘K, `?words` searches chunks and Enter opens one in the web app; dropping a file on
  the window shows its `/api/context/for-file` context. Ended AI sessions have "Save as note".
- **Notifications** (macOS) when a process goes down without you stopping it, starts failing its health check, or a Claude Code
  session finishes its turn and is waiting for you.
- **Terminal** (⌘0): embedded shells built on Ghostty's VT engine (`libghostty-vt` 0.2.2, pinned; pre-1.0) and a PTY
  (`portable-pty`). Each repo has a strip of terminal tabs (its own and its worktrees'): "+ Shell", "Shell" on a worktree or
  in ⌘K, ✕ to close, "Split" for a second shell beside the active one. Colors and styles, the cursor, special keys and ctrl
  combinations through Ghostty's key encoder (cursor-key mode and the Kitty keyboard protocol follow the program); typed
  text through GPUI's input handler, so IME composition (shown underlined at the cursor) and dead keys work. Option types
  characters (GPUI does not report which Option key is held, so it is not used as Alt). ⌘V pastes (bracketed when asked),
  ⌘F searches the scrollback (⏎ / ⇧⏎ step), ⌘-click opens a URL. Mouse: drag selects, double click a word, triple click a
  line, ⌘C or "Copy" copies; when a program tracks the mouse (vim, htop, Claude's menus) clicks, drags, right/middle
  buttons and the wheel go to it (hold ⇧ to select anyway). Runs are drawn at their exact cell width so wide and fallback
  glyphs keep columns aligned. Output wakes the view (no timer per tab), and frames are rebuilt only when something changed.
- **tmux persistence** (default when tmux is installed; `terminal_persistence = "none"` turns it off): every terminal tab
  is a session on a private tmux server, `tmux -L fubbik`, with its own minimal config (`~/.config/fubbik/tmux.conf`: no
  status bar, no escape delay, 50 000 lines of history, true color) — your own tmux server and `tmux.conf` are untouched.
  Quitting the app only detaches: shells and agents keep running, and the next launch reattaches them as tabs (label and
  directory are kept as session options). Closing a tab with ✕ ends its session. The wheel scrolls tmux's copy mode, since
  history lives in tmux. Plan tasks also get "Start in background": worktree, board claim and Claude start without leaving
  the current view.
- **Agents in tmux, supervised:** AI Sessions matches each live session to the tmux pane it runs in — on the app's server
  or yours — by process ancestry (Claude Code) or command and directory (Codex), and shows the last lines of its screen.
  "Reply" types a message into that pane and presses Enter; "Open" switches to its tab, or attaches to your own tmux
  session in a new tab.
- **AI sessions run in the Terminal tab:** "New Claude", "Resume" and "Start task" open a labelled terminal tab in the right
  directory and type the command into a login shell (so the shell stays when the session ends). `terminal = "external"` in
  the desktop config sends them to Terminal.app instead. Building needs Zig 0.15.2 first on PATH (`brew install zig@0.15`; `just start-desktop` and
  `just desktop-install` set this up). `FUBBIK_DESKTOP_TAB=terminal` opens the app on that tab (any tab name works).
- **Ghostty themes:** ⌘K → "Theme: …" (or View → Choose Theme…) lists every Ghostty theme — `~/.config/ghostty/themes` first,
  then the ones bundled with Ghostty.app — plus "Built-in" and "Follow Ghostty config". Saved as `ghostty_theme` in the
  desktop config (a theme name, `"follow"`, or unset). A theme gives the terminal its full palette, default colors, cursor
  and selection colors; the UI derives from it (surfaces step from the background toward the foreground, text is the
  foreground, accent and status colors are its ANSI blue, green, yellow, red). "Follow" reads `~/.config/ghostty/config` the
  way Ghostty does: its `theme` (including `light:X,dark:Y`, picked by the macOS appearance), then its own color keys on top,
  plus `font-family`, `font-size` and `cursor-style = bar` for the terminal (a font that is not installed falls back to
  Menlo). ⌘R re-reads the Ghostty config.
- **Translucent theme:** ⇧⌘T, View → Toggle Translucent Theme, or "Theme" in ⌘K (saved as `theme = "translucent"` in the
  desktop config). The window shows the desktop through the system blur (`WindowBackgroundAppearance::Blurred`) with tinted
  glass panels; the palette, context panel and toasts stay near-opaque. Both themes draw under a transparent titlebar: the
  top strip holds only the title, so the window drags from it.
- **Today** (⇧⌘H; the home screen once repos exist): across all repos, what needs you (waiting agents, failing processes,
  failing requirements or behaviour cells, stale knowledge, worktrees changing the same files, broken repos), tasks in
  flight (holder, time in progress, tokens) and the next startable tasks (dependencies done, unclaimed) with "Start" and
  "In background".
- **Plan board** (Plans tab): one plan at a time in columns — Pending, In progress, Blocked, Done. Cards show what a task
  waits on and unblocks, its acceptance criteria as a checklist you can tick (saved to fubbik), who holds it and what it
  cost (sessions, tokens, time); ◀ ▶ move it, "Block", "Start" (once its dependencies are done) and "Timeline" (created,
  claimed, board notes, commits on its branch, merged).
- **Task to merge:** "Start in new worktree" records the task's branch and main's commit at the time in the task's
  metadata (`fubbikDesktop`). When the branch's work lands on main, the task is marked done, its worktree removed if clean
  (git refuses a dirty one), and a toast offers to save a "what changed" note.
- **Knowledge graph** (Knowledge tab): the repo's space as a graph (`GET /api/graph`, laid out in `src/graph_layout.rs`):
  chunks as nodes colored by type and sized by how connected they are, connections as edges; orphan chunks line up in a
  strip along the bottom. Hover lights up a chunk's neighbours, click selects it and lists its connections (each a click
  to that chunk) with "Open" for the web app. Type chips hide or show a type; "Labels" labels every node. "Color: Attention"
  colors by what needs attention (flagged: a changed file or diverged duplicate; stale; thin; orphan; healthy). The search
  field lights up matching chunks and ⏎ selects the first (in 3D, turning the camera to it). With a chunk selected,
  shift-click another to connect them: pick the relation in the panel and Connect (`POST /api/connections`). **3D** lays the same graph out in three
  dimensions (orphans on a shell around it) and projects it with perspective: drag to turn, scroll to zoom, "Reset view";
  it turns slowly on its own ("Auto-rotate") while nothing is hovered. Nearer chunks are bigger, brighter and on top.
- **Insights** (⇧⌘I): a standup (1 day) or weekly summary built from git, plan tasks, agent sessions and requirements —
  copy it as Markdown or save it as a chunk; a health score (0–100) with the reasons it lost points (failing processes or
  requirements, violated cells, vulnerabilities from `cargo audit` / `pnpm audit` when "Run audit" was used, stale
  branches, idle worktrees, stale knowledge, low knowledge coverage); a 4-week heatmap of commits and agent sessions; stale
  branches (30+ days) and idle worktrees (7+ days).
- **Agent limit:** Preferences → "Agents at once" (`max_agents`); launches beyond it wait in a queue and start as slots
  free. Worktrees changing the same files are flagged on their rows and in Today.
- **Scripts and profiles** (Processes tab): one-click `just` recipes, `package.json` scripts and Makefile targets (output in
  Logs; also in ⌘K), and run profiles from the services file — `[profile.dev] processes = ["api", "web"]` starts exactly
  those and stops the repo's others.
- **Quick capture** (⇧⌘N): a note into the selected repo's fubbik space from anywhere.
- **Changes** (⇧⌘G) has three views. **History**: the branch's last 80 commits with their refs, Copy SHA and GitHub.
  **Branches**: local and remote branches, most recent first, with a filter; Switch (a remote one is checked out as a new
  tracking branch; git refuses rather than overwrite changes) and New worktree. **Files**: the changed files of the repo or any of its worktrees, staged and not, with each file's colored diff;
  stage and unstage files (or all), and commit what is staged. Beside the commit box: the fubbik chunks related to the
  staged files (`fubbik --json check-files --staged`), so the knowledge for a change is in view before it is committed.
- **Sync with the remote** (`src/ui/sync.rs`): every repo with a remote is fetched quietly every 15 min (prompts off,
  so it never hangs on a password), so ahead/behind is current. The repo header shows "↓ Pull N" (fetch, then
  `git merge --ff-only @{u}`: never a rebase or merge — a diverged branch is left alone with a message), "↑ Push N"
  (asks first, then runs `git push`, or `git push -u origin <branch>` the first time, in a terminal tab where hooks
  and credential prompts show) and "Fetch". Home cards show "↑N unpushed" (amber from 10). Also in ⌘K.
- **Daily digest** (`src/digest.rs`): Today's "Digest" shows the last 24 hours per repo from the change feed and open
  alerts (commits, PRs, agents, processes down, worktrees, dependency drift, tool changes); "Save to fubbik" writes each
  repo's part as a note in its space, and the app does so on its own once a day after 8:00. Agents get it as `get_digest`.
- **Pull requests** (`gh`, repos with a GitHub remote, read every 3 min and on Refresh): each worktree's branch shows its
  latest PR as a pill (checks, review, conflicts) with a "PR #n" button; the Changes tab shows it for the checkout picked.
  An open PR with failing checks, changes requested or conflicts is a "Needs you" line on Today.
- **Live tmux events:** global hooks on the app's tmux server (`alert-bell`, `pane-died`) append to
  `~/.config/fubbik/tmux-events.log`, read every 300 ms. A bell (`\a`) in a terminal tab raises a toast and a "Needs you"
  line until the tab is looked at; a pane that exits re-polls at once, so a run's exit shows straight away.
- **Worktree review:** "Review" on a worktree shows its commits, diffstat and colored diff against main, with "Open PR"
  (`gh pr create --fill`) and "Merge into main" (`git merge --no-ff`), each run in a terminal tab.
- **Codex waiting:** Codex has no live-session registry, so a Codex screen that changed and then stayed still for three
  polls counts as waiting for input, with a notification like Claude's.
- **Alerts:** notifications also raise a toast; clicking it jumps to the repo and tab it is about.
- **Preferences** at the top of Setup (and on the first-launch screen, with a getting-started checklist): colors,
  translucency, tmux persistence, where AI sessions open, and the fubbik server, web app and binary.
- **Events over timers:** file changes in a repo's root or Claude's live-session registry trigger a poll at once (300 ms
  debounce); the 1 s tick only redraws for toasts and the live log; knowledge fetches back off while the fubbik API is
  down (60 s, doubling to 5 min).
- **Keys:** ⌘/ shows every shortcut, and ⌘K lists each command's key beside it. Tabs: ⌘1 Today, ⌘2 Overview, ⌘3 Processes,
  ⌘4 Logs, ⌘5 Worktrees, ⌘6 Sessions, ⌘7 Plans, ⌘8 Knowledge, ⌘9 Terminal, ⇧⌘D Databases, ⇧⌘I Insights, ⇧⌘G Changes, ⇧⌘A Agents, ⇧⌘T Tools, ⇧⌘L Dependencies, ⌘,
  Setup. ⌃1–⌃9 pick a repo, ⇧⌘J jumps to the next thing that needs you, ⇧⌘R replies to the agent that has waited longest,
  ⇧⌘S starts the next ready task in the background, ⇧⌘N captures a note. In a terminal: ⌘T new shell, ⌘W close, ⌘1–⌘9 and
  ⇧⌘[ / ⇧⌘] switch tabs, ⌘D split, ⇧⌘K clear. Rebind any of them in `~/.config/fubbik/keymap.toml` (Setup → Keyboard
  writes a commented starter; `""` unbinds; conflicts and unknown actions are listed there).
- **Keyboard lists:** when no text field or terminal has focus, j/k (↓/↑) move a highlight in Today, Processes, Worktrees,
  AI Sessions and the Knowledge inbox, ⏎ does the row's main thing, and letters do the rest (Processes: s start/stop, r
  restart, l logs, o open; Worktrees: v review, t shell, n session, x remove; Knowledge: d dismiss, a/r approve/reject). On
  the plan board, h/l pick a column, j/k a card, ←/→ move it between columns, space opens its checklist, s starts it, b
  blocks it. ⇧⌘E hands j/k to the sidebar until ⏎ or esc. A hint bar under each view names its keys.
- **Palette prefixes:** in ⌘K, `@` lists repos and their tabs, `#` open tasks on every plan, `!` running processes (⏎ shows
  the log) and live agents (⏎ opens the pane), `>` commands only, `?` searches chunks. With nothing typed, the commands you
  ran last come first.
- **Summon key:** ⌃⌥F shows fubbik from any app, and hides it when it is already in front. Change it with `global_hotkey =
  "cmd-shift-space"` in `desktop.toml`, or turn it off with `global_hotkey = ""` (macOS, through Carbon's
  `RegisterEventHotKey`; no accessibility permission needed). Restart to apply.

Not built: a menu-bar (status bar) icon. GPUI has no API for one.

## Tests

`cargo test` in `apps/desktop` (with Zig 0.15.2 on PATH) runs the unit tests and UI tests that drive a real `Workspace` in
GPUI's test harness (`src/ui/ui_tests.rs`: actions, simulated keystrokes into a real shell, the file watcher, …). Tests that
need tmux skip without it. CI runs format, Clippy (`-D warnings`) and the tests on macOS (`.github/workflows/desktop.yml`).
`cargo test claude_tui -- --ignored --nocapture` checks Claude Code's TUI renders in the terminal (needs `claude`).

## Layout

```
src/main.rs        window, menus, key bindings, --probe; quitting stops child processes
src/config.rs      AppConfig / RepoEntry: load, save, add, remove
src/services.rs    fubbik.services.toml: ProcessSpec, DatabaseSpec
src/detect.rs      starter services file from package.json and compose
src/git.rs         porcelain v2 status, changed files, diffs, stage / unstage / commit, remote → web URL
src/github.rs      pull requests and CI checks per branch (gh pr list)
src/tmux_events.rs tmux hooks → events file (bells, pane exits), read incrementally
src/mcp_events.rs  the MCP change feed: diffs between published views, pushed events, deps and tools watchers
src/mcp_more.rs    MCP tools for a tracker agent: changes since, dependencies, toolchain, PRs, worktrees, changes, diffs
src/tracker.rs     notes, todos, decisions and watches (alerts) for a tracker agent
src/graph_layout.rs  the knowledge graph: nodes and edges from /api/graph, force-directed layout
src/tools.rs       developer tools and services: versions, paths, brew outdated, recent upgrades
src/deps.rs        manifests across repos: declared dependency versions and toolchain pins, compared
src/probe.rs       port, owner PID, HTTP health, TCP, compose, ps
src/supervisor.rs  child processes: process groups, log ring buffers
src/fubbik.rs      fubbik API client (space, knowledge, quick note)
src/worktree.rs    git worktree list / create / remove / prune, ahead-behind main
src/sessions.rs    Claude Code and Codex session discovery (SessionIndex keeps read offsets)
src/agent_task.rs  start a Claude session on a plan task (worktree, board run, claim, prompt)
src/setup.rs       Rust binary lookup, MCP config, pre-commit hook
src/claude_md.rs   fubbik.config.json claudeMd, CLAUDE.md state and atomic write
src/gaps.rs        uncovered source files
src/activity.rs    git activity, heatmap, stale branches, audits, health score, standup summary, timestamps
src/scripts.rs     just / package.json / make script detection
src/ghostty.rs     Ghostty theme and config parsing, theme lookup, palette resolution
src/tmux.rs        the app's tmux server: sessions, panes, capture, send-keys, copy-mode scroll
src/term.rs        TermSession: PTY + libghostty-vt Terminal, frames of styled runs, key encoding
src/model.rs       Status, ProcessState, DatabaseState, RepoSnapshot, roll-up
src/poll.rs        one repo poll (runs on the background executor)
src/launch.rs      editor, Terminal, notifications, login-shell PATH
src/global_hotkey.rs  the system-wide summon key (Carbon RegisterEventHotKey)
src/menu_bar.rs    the menu bar item (NSStatusItem) and its click channel
src/claude_hooks.rs  Claude Code hooks in ~/.claude/settings.json: install, uninstall, parse a hook's report
src/proxy.rs       .test domains: Caddyfile block, /etc/hosts entries, port conflicts
src/env_drift.rs   .env key names vs .env.example (values never kept)
src/test_runs.rs   test runners (nextest, cargo test, vitest) and their failures
src/containers.rs  Docker containers per repo, docker system df
src/pg.rs          PostgreSQL details, migration drift, psql, snapshots and restore
src/disk.rs        build output and packages per repo: sizes, cleaning
src/digest.rs      the daily digest from the change feed
src/tmux_runs.rs   processes and scripts in tmux sessions (fk-run-*), their logs and exit codes
src/system.rs      the computer's stats for Home (CPU, memory, disk, battery, network, top processes, ports)
src/mcp.rs         the app's MCP server (HTTP on :3100, --mcp stdio bridge), answering from ui/mcp_publish.rs
src/backend.rs     the built-in server: Postgres cluster, in-process API, import, backups, server.json, --serve
src/ui/            Workspace entity (mod.rs: state; polling, commands, command_palette, fields, terminals,
                                      chrome, preferences, tasks, today, insights, changes, prs, tmux_live, tools_view, deps_view), sidebar, tab views (agents.rs: worktrees + sessions; plans.rs; knowledge.rs;
                   setup_view.rs; terminal_view.rs), actions.rs (fubbik calls and scans), palette, text field, widgets,
                   keymap.rs (bindings + keymap.toml), hotkeys.rs, lists.rs (keyboard lists)
```
