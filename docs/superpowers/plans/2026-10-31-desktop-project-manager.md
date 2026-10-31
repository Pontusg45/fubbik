# fubbik desktop: Project Manager Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to
> implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the `apps/desktop` GPUI shell into a working local project manager: registered repos, live process / database / git
status, process control with logs, fubbik knowledge for each repo, and keyboard-first navigation.

**Architecture:** Pure, unit-tested modules (config, services file, detection, git parsing, probes, supervisor, fubbik client) sit
under a GPUI layer. One `Workspace` entity owns the app state. A poll loop (`cx.spawn` + `background_executor().timer`) refreshes
probes on the background executor every 3 s and pushes results back with `this.update`. Processes started by the app are children
held by a `Supervisor`, whose stdout and stderr fill per-process ring buffers.

**Tech Stack:** Rust 2024 (toolchain 1.96), gpui 0.2.2 (`runtime_shaders`), serde + toml + serde_json, ureq 3 without TLS (local
HTTP only), tempfile (tests). Shell tools: `git`, `lsof`, `ps`, `docker`, `osascript`, `open`.

**Spec:** the feature list agreed in chat on 2026-10-31 (sections 1–4: core loop, control, fubbik integration, quality of life). The
user chose "build all, no stops".

## Global Constraints

- The app stays its own cargo workspace in `apps/desktop`. Nothing in `crates/` depends on it, and it adds nothing to the root `Cargo.toml`.
- No TLS stack and no async runtime crate. GPUI's executors and blocking calls on the background executor are enough.
- App config: `$HOME/.config/fubbik/desktop.toml`, or `FUBBIK_DESKTOP_CONFIG` if set.
- Services file for each repo: `<repo>/fubbik.services.toml` (`.fubbik/` is gitignored and holds local data).
- fubbik API URL: the config's `fubbik_url`, else `FUBBIK_URL`, else `http://localhost:3000`. An unreachable API means "no knowledge
  panel". It never shows as an error state for the repo.
- Status poll interval: 3 s. Log ring buffer: 2 000 lines per process. CPU history: 60 samples.
- No commits: the user has not asked for any (global CLAUDE.md).

## Review Focus

1. A repo path that no longer exists → the sidebar shows it as missing, and the app neither panics nor drops the repo from the config.
2. A malformed `services.toml` → the repo shows the parse error inline, and the other repos keep working.
3. Stopping a process that spawned children (`pnpm dev` → vite) → the whole process group dies, and no orphan holds the port.
4. A port held by a process the app did not start → reported as "external" with its PID, not as "running (ours)".
5. The app quitting while it has children running → the children are terminated (the Supervisor kills its process groups on drop).

---

## File map

```
apps/desktop/
  .fubbik/…            (none — the example services file lives in the fubbik repo root: /.fubbik/services.toml)
  src/main.rs          window, menus, key bindings, quit (kills children)
  src/config.rs        AppConfig { fubbik_url, repos: Vec<RepoEntry { path, group }> }: load / save / add / remove
  src/services.rs      ServicesFile { process: Vec<ProcessSpec>, database: Vec<DatabaseSpec> }: parse / load / save
  src/detect.rs        detect(repo) -> ServicesFile from package.json scripts and docker-compose.yml
  src/git.rs           parse_status(porcelain v2) -> GitInfo; read(repo) -> Result<GitInfo>; remote_web_url
  src/probe.rs         port_listening, port_owner, http_ok, tcp_reachable, compose_state, process_stats
  src/supervisor.rs    Supervisor: start / stop / restart / logs / is_running, process groups, ring buffers
  src/fubbik.rs        FubbikClient: detect_space, knowledge(space_id), quick_note; JSON parsing kept pure
  src/model.rs         Repo + runtime ProcessState / DatabaseState / Status roll-up (replaces project.rs)
  src/poll.rs          snapshot(repo, supervisor) -> RepoSnapshot (runs on the background executor)
  src/actions.rs       open in editor / terminal / browser / GitHub, notifications
  src/theme.rs         colors
  src/ui/mod.rs        Workspace entity, poll loop, top-level render
  src/ui/sidebar.rs    grouped repo list, register / remove, group start / stop
  src/ui/views.rs      Overview, Processes, Databases, Logs, Knowledge tabs
  src/ui/palette.rs    ⌘K command palette (typed query, fuzzy filter, enter runs)
  src/ui/widgets.rs    status_dot, status_pill, stat_card, section, sparkline, button
```

### Task 1: Config and services file

**Files:** Create `src/config.rs`, `src/services.rs`. Modify `Cargo.toml` (serde, toml, serde_json, ureq, tempfile dev-dep).

**Interfaces — Produces:**
- `AppConfig { fubbik_url: Option<String>, repos: Vec<RepoEntry> }`, `RepoEntry { path: PathBuf, group: Option<String> }`
- `config::path() -> PathBuf`, `AppConfig::load_from(&Path) -> Result<AppConfig, String>` (missing file → default),
  `AppConfig::save_to(&self, &Path) -> Result<(), String>`, `add_repo(&mut self, PathBuf) -> bool` (false if already present),
  `remove_repo(&mut self, &Path)`
- `ProcessSpec { name, command, cwd: Option<String>, port: Option<u16>, health_url: Option<String> }`,
  `DatabaseSpec { name, engine: Option<String>, url: String, compose_service: Option<String>, migrate: Option<String>, seed: Option<String>, studio: Option<String> }`,
  `ServicesFile::parse(&str) -> Result<ServicesFile, String>`, `ServicesFile::load(repo: &Path) -> Result<Option<ServicesFile>, String>`, `save(repo)`

- [x] Tests: `round_trips_repos`, `missing_file_is_default`, `add_repo_dedupes`, `parses_processes_and_databases`, `rejects_unknown_keys` (`deny_unknown_fields`), `missing_services_file_is_none`
- [x] Implement, `cargo test` passes

### Task 2: Detection

**Files:** Create `src/detect.rs`. **Produces:** `detect(repo: &Path) -> ServicesFile`.

Rules: `package.json` scripts named `dev`, `start`, or starting with `dev:` → `ProcessSpec { command: "<pm> run <script>" }`, with the package manager taken from the lockfile (pnpm / bun / yarn / npm). A `docker-compose.yml` / `compose.yml` service whose image contains `postgres`, `mysql`, `mariadb`, `redis` or `mongo` → `DatabaseSpec` with `compose_service`, and its URL built from the first published port. Compose is scanned line by line (services, `image:`, `ports:`), with no YAML crate.

- [x] Tests: `detects_pnpm_dev_scripts`, `detects_compose_postgres_with_port`, `empty_repo_detects_nothing`

### Task 3: Git

**Files:** Create `src/git.rs`. **Produces:** `GitInfo { branch, head, changed_files, ahead, behind, last_commit: String }`,
`parse_status(&str) -> GitInfo` (porcelain v2 `--branch`), `read(repo) -> Result<GitInfo, String>`, `remote_web_url(repo) -> Option<String>` (git@ / https remote → https URL).

- [x] Tests: `parses_branch_head_ahead_behind`, `counts_changed_and_untracked`, `detached_head`, `ssh_remote_to_https`

### Task 4: Probes

**Files:** Create `src/probe.rs`. **Produces:** `port_listening(u16) -> bool`, `port_owner(u16) -> Option<u32>` (`lsof -nP -iTCP:<p> -sTCP:LISTEN -t`),
`http_ok(&str) -> bool` (2xx within 1 s), `tcp_reachable(host_port_from_url(&str)) -> bool`, `host_port_from_url(&str) -> Option<(String, u16)>`,
`compose_running(repo, service) -> Option<bool>`, `process_stats(pid) -> Option<(f32 cpu, u64 rss_kb)>`.

- [x] Tests: `listening_port_detected` (bind a `TcpListener` on 0), `free_port_not_listening`, `parses_postgres_url_host_port` (default 5432), `http_ok_against_local_server`

### Task 5: Supervisor

**Files:** Create `src/supervisor.rs`. **Produces:** `Supervisor::new()`, `start(&self, key: &str, command: &str, cwd: &Path) -> Result<u32, String>`,
`stop(&self, key)`, `restart`, `is_running(key) -> bool`, `pid(key) -> Option<u32>`, `logs(key) -> Vec<String>`, `clear_logs(key)`.
`Supervisor` is `Clone` (`Arc<Mutex<…>>`). It spawns `sh -c` with `process_group(0)`, stops with `kill -TERM -<pgid>` and then KILL after 2 s, and `Drop` of the last clone stops all.
The key is `"<repo path>::<process name>"`.

- [x] Tests: `captures_stdout_and_stderr`, `stop_kills_process_group` (`sh -c 'sleep 30 & sleep 30'`, both gone), `ring_buffer_caps_lines`, `exited_process_not_running`

### Task 6: fubbik client

**Files:** Create `src/fubbik.rs`. **Produces:** `FubbikClient::new(base)`, `detect_space(remote: Option<&str>, local_path: &Path) -> Option<Space { id, name }>`,
`knowledge(space_id) -> Result<Knowledge, String>`, `quick_note(space_id, title, content) -> Result<(), String>`.
`Knowledge { chunks: i64, stale: i64, orphans: i64, thin: i64, plans: Vec<PlanSummary { id, title, status, task_done, task_total }>, requirements: ReqStats { total, passing, failing, untested }, active_agents: usize }`.
Endpoints: `GET /api/spaces/detect`, `/api/chunks?spaceId=&limit=1` (`total`), `/api/chunks/stale/count` (text), `/api/health/knowledge?spaceId=`
(`orphans.count`, `thin.count`), `/api/plans?spaceId=`, `/api/requirements/stats?spaceId=`, and `/api/plans/{id}/board` for in-progress plans (runs with `status == "active"`).
`POST /api/chunks { title, content, type: "note", spaceIds: [id] }`. 404s on feature-gated routes count as zero.

- [x] Tests (pure parsers): `parses_plan_rows`, `parses_requirement_stats`, `counts_active_runs`, `empty_detect_body_is_none`

### Task 7: Model and poll

**Files:** Create `src/model.rs`, `src/poll.rs`. Delete `src/project.rs`.
**Produces:** `Status { Running, Degraded, Stopped, External, Error }`, `ProcessState { spec, status, pid, owned: bool, cpu_history: Vec<f32>, rss_kb }`,
`DatabaseState { spec, status }`, `RepoSnapshot { git: Result<GitInfo,String>, processes, databases, services_error: Option<String>, missing: bool }`,
`roll_up(&[Status]) -> Status`, `poll::snapshot(entry: &RepoEntry, sup: &Supervisor, prev: Option<&RepoSnapshot>) -> RepoSnapshot`.
Process status: owned and running → Running. Port listening but not owned → External. Health URL failing while listening → Degraded. Otherwise Stopped.

- [x] Tests: `roll_up_all_running`, `roll_up_partial_is_degraded`, `external_counts_as_up`, `missing_repo_snapshot`, `bad_services_file_reports_error`

### Task 8: UI — workspace, sidebar, views, actions

**Files:** Create `src/ui/{mod,sidebar,views,widgets}.rs` and `src/actions.rs`. Modify `src/main.rs`, `src/theme.rs`.
Sidebar: repos grouped by `group` (ungrouped first), a missing or errored marker, "+ Register repo" (`prompt_for_paths`, directories only, then save the config), remove (×), and start / stop all for each group.
Header: path, git, Open in Editor (`$VISUAL`/`$EDITOR`, else `zed`, else `code`), Terminal (`open -a Terminal`), GitHub (`remote_web_url`).
Tabs: Overview, Processes (start / stop / restart, port, PID, owned/external, CPU sparkline, RSS, "kill external" for a port conflict, open `http://localhost:<port>`), Databases (state, copy URL, start/stop compose service, migrate / seed / studio buttons run through the Supervisor), Logs (process picker, last 2 000 lines, pause, clear, filter text), Knowledge.
No services file → an empty state with "Generate from repo" (detect + save).
Notifications (`osascript display notification`): a process that was Running becomes Stopped without the user stopping it, or a health check goes from ok to failing.

### Task 9: fubbik panel and command palette

**Files:** Create `src/ui/palette.rs`. Modify `src/ui/views.rs` and `src/ui/mod.rs`.
Knowledge tab: space name, chunks / stale / orphans / thin, plans with progress bars, requirement counts, active agents, and quick note (title field + content field) → `quick_note`.
Palette (⌘K): actions such as "Go to <repo>", "Start <proc>", "Stop <proc>", "Start all", "Stop all", "Register repo", "Open in editor", "Tab: <tab>". The typed query filters by subsequence match, ↑/↓ select, enter runs, esc closes. Key bindings: ⌘K palette, ⌘1–⌘5 tabs, ⌘[ / ⌘] previous / next repo, ⌘Q quit.
Text input is a minimal focused element that handles `on_key_down` (printable chars, backspace). No IME and no selection.

### Task 10: Example services file, docs, verification

**Files:** Create `/.fubbik/services.toml` at the fubbik repo root (api, web, ollama; postgres via the compose service with migrate/seed/studio; test-db). Update `apps/desktop/README.md`.
- [x] `cargo test`, `cargo clippy`, `cargo fmt --check` clean in `apps/desktop`
- [x] Launch, confirm no panics in stderr for 10 s

**Not built (and why):** a menu-bar status item. GPUI 0.2.2 has no API for `NSStatusItem`, and adding raw Cocoa bindings for it is a separate project. Notifications cover the "tell me when something breaks" part of it.
