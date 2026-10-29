# Fork Modularity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make fubbik easy to fork and adapt. Personal settings come out of the code, domains can be switched off without code
changes, the standalone tools can be used without forking, review rules and agent material become configurable, the retired
TypeScript code becomes removable, and the docs explain how to fork and extend.

**Architecture:** A new `fubbik-config` crate reads an optional, committed `fubbik.toml` (project-level choices such as which feature
groups are on). Personal machine settings (hostnames, paths) live in the gitignored root `.env`. The API mounts only enabled feature
groups and reports them at `GET /api/capabilities`, and the web nav hides disabled groups. The remaining tasks are self-contained:
moving files, adding small CLI subcommands, and writing docs.

**Tech Stack:** Rust 2024 (axum, serde, `toml`, clap), TanStack Start web app (TypeScript), pnpm, Just, GitHub Actions.

**Spec:** The nine improvement suggestions from the conversation on 2026-10-29, which the user approved with "fix all". They're
summarised in the Decisions section, because no separate spec file exists.

## Decisions

These depart from or settle points in the suggestions. The user can override any of them before execution.

1. **Features are switched on and off at runtime, not with Cargo features.** `fubbik.toml` `[features]` toggles route groups at server
   start, so a fork changes behaviour without recompiling, and the web nav follows the same switches. Compile-time Cargo features are
   left out (YAGNI): they would add a build matrix without giving forks anything extra.
2. **The database schema is always complete.** Disabling a feature only stops its routes and background jobs. Migrations stay linear and
   unconditional, because gating sqlx migrations would fork the schema.
3. **Settings are split across two files.** Personal settings go in `.env` (already gitignored and documented in `.env.example`).
   Project choices go in `fubbik.toml` (committed, optional, with all features on when it's absent).
4. **CLAUDE.md stays hand-written.** Generating it from chunks is documented as an option in `docs/forking.md` but not switched on,
   because auto-rewriting the project's main agent instructions is too risky for this plan. Task 8 rewrites its stale architecture
   sections by hand instead.
5. **`apps/cli` and `packages/mcp` (TypeScript) move to `legacy/`.** The Rust `fubbik` binary and `crates/fubbik-mcp` replace them.
   `packages/db` stays, because Drizzle Studio, the typed seed adapter and Rust tests `include_str!` its SQL.
6. **The seed starter is the existing `minimal` scenario.** No new scenario is added. Task 5 refreshes the demo catalog's moved paths, and
   the docs name `fubbik seed --scenario minimal` as the neutral starter.
7. **There is no `fk fork-setup` script.** Forking is a documented checklist (`docs/forking.md`) plus a CI job that proves `legacy/` can
   be deleted. A script that rewrites a fork automatically would be the most fragile piece in the repo.

## Feature groups

The `core` group is always on and has no switch. It covers every router not listed below.

| Key in `[features]` | Routers mounted (from `crates/fubbik-api/src/lib.rs` `router()`) | Requires |
| --- | --- | --- |
| `plans` | `plans::routes`, `tasks` | — |
| `requirements` | `requirements::routes`, `requirements::dependency_routes`, `use_cases::routes`, `coverage::routes` | — |
| `matrices` | `matrices::routes` (and the behavior-graph sync job in `startup.ts`'s Rust equivalent) | `requirements` |
| `coordination` | `coordination::routes` | `plans` |
| `learning_paths` | `learning_paths::routes` | — |
| `overlays` | `features::routes` (knowledge-overlay features) | — |
| `ai` | `ai::routes` | — |

## Global Constraints

- Work in the worktree `.claude/worktrees/fork-modularity` on branch `chore/fork-modularity`. **Never run `git push`.**
- Stage only the paths a task names, by name. Never `git add -A` or `git add .`. Conventional commit messages, with no co-author trailer.
- Use `git mv` for moves of tracked files.
- Rust test bodies use `// Given`, `// When`, `// Then` comments.
- After every task: `cargo fmt --all --check` passes, `just rust-test-fast` passes, and for tasks that touch TypeScript,
  `pnpm run check-types` passes.
- **Never run real port kills** (`just kill-dev`, `pnpm kill:*`, `fk port kill` without `--dry-run`). Never touch any `.env` file;
  only `.env.example` is edited.
- Verification greps exclude `node_modules`, `target`, `.git`, `.superpowers` and `docs/superpowers`.

## Review Focus

1. **With no `fubbik.toml`, every feature is on.** An existing install must behave exactly as before. Test:
   `missing_file_enables_everything` (Task 1).
2. **A typo in a feature key is an error that lists the valid keys.** It must not be silently ignored. Test:
   `unknown_feature_key_is_rejected` (Task 1).
3. **A web UI talking to an older server without `/api/capabilities` shows every nav item**, rather than an empty nav. Test:
   `nav_falls_back_to_all_items_when_capabilities_fail` (Task 2).
4. **Running `fk agents link` twice is a no-op.** It never replaces a real (non-symlink) file in `.claude/`. Tests:
   `link_is_idempotent` and `link_refuses_to_replace_a_real_file` (Task 7).
5. **Deleting `legacy/` leaves the workspace building and type-checking.** The CI job `legacy-removable` covers this (Task 4).

---

### Task 1: `fubbik-config` crate, feature-gated routes, and `GET /api/capabilities`

**Files:**
- Create: `crates/fubbik-config/{Cargo.toml,src/lib.rs}` and `fubbik.example.toml`
- Modify: `crates/fubbik-api/src/lib.rs` (`AppState`, `router()`), `crates/fubbik-api/Cargo.toml`, `crates/fubbik/src/main.rs` (load the
  config at `serve`; skip the behavior-graph sync job when `matrices` is off), `Cargo.toml` (add `toml = "0.8"` to
  `[workspace.dependencies]`), `openapi.json`, `packages/client/src/api-types.ts` (regenerated)
- Test: `crates/fubbik-config/src/lib.rs` (unit tests), `crates/fubbik-api/tests/capabilities.rs`

**Interfaces:**
- Produces, in `fubbik-config`:
  - `pub enum Feature { Plans, Requirements, Matrices, Coordination, LearningPaths, Overlays, Ai }`, with `Feature::ALL` and
    `fn key(self) -> &'static str`. The keys are exactly the `[features]` keys in the table above.
  - `pub struct Features` (Default = all on), with `fn enabled(&self, f: Feature) -> bool` and
    `fn enabled_keys(&self) -> Vec<&'static str>` (in `Feature::ALL` order).
  - `pub struct Config { pub features: Features }`, with `Config::from_toml_str(s: &str) -> Result<Config, ConfigError>` and
    `Config::load() -> Result<Config, ConfigError>`. `load` reads the path in `FUBBIK_CONFIG` if set, otherwise `./fubbik.toml` if it
    exists, otherwise returns the default.
  - `pub enum ConfigError` (`thiserror`). Its messages are exact:
    - `unknown feature "<key>"; valid features: plans, requirements, matrices, coordination, learning_paths, overlays, ai`
    - `feature "<a>" requires "<b>"`
    - `invalid fubbik.toml: <toml error>`
    - `cannot read <path>: <io error>`
- Produces, in `fubbik-api`: an `AppState.features: fubbik_config::Features` field. `router()` mounts a group's routers only when it's
  enabled. `GET /api/capabilities` is always mounted, needs no auth, and returns `{"features": [<enabled keys>]}`, registered in
  OpenAPI.

- [ ] **Step 1: Write the failing unit tests in `fubbik-config`**

| Test | Assertion |
| --- | --- |
| `missing_file_enables_everything` | with `FUBBIK_CONFIG` unset and the current dir a temp dir with no file, `load()` gives every feature enabled |
| `empty_features_table_enables_everything` | `from_toml_str("[features]\n")` gives all enabled |
| `false_disables_one_feature` | `ai = false` gives `!enabled(Ai)`, and every other feature is still enabled |
| `unknown_feature_key_is_rejected` | `plan = false` errors with exactly the unknown-feature message above for `plan` |
| `dependency_is_enforced` | `requirements = false` with matrices on errors with `feature "matrices" requires "requirements"` |
| `fubbik_config_env_points_at_a_file` | `FUBBIK_CONFIG=<temp file>` with `overlays = false` gives `!enabled(Overlays)` |

Tests that touch the process environment or current dir must be serialised. Use one test that runs the env/cwd cases in sequence, or
run them under a `std::sync::Mutex`.

- [ ] **Step 2: Run the tests to check they fail**

Run: `cargo test -p fubbik-config`
Expected: compile errors for the missing items

- [ ] **Step 3: Implement `fubbik-config`**

Parse with `toml` into a `BTreeMap<String, bool>` under `features`, so unknown keys can be detected and named. Then validate the
dependencies from the table above.

- [ ] **Step 4: Write the failing API test `crates/fubbik-api/tests/capabilities.rs`**

Follow the existing test harness pattern in `crates/fubbik-api/tests/` for building an `AppState`/`router`. Read one test file there to
copy it. Tests:
- `capabilities_lists_enabled_features`: default features give 200 and all seven keys in table order.
- `disabled_group_routes_are_not_mounted`: with `ai` off, `GET /api/capabilities` omits `ai`, and the first route in `ai::routes`
  returns 404.

If the harness needs a database, these tests run under `just rust-test`, not `rust-test-fast`. Say so in the report and run them
there.

- [ ] **Step 5: Implement the gating and the endpoint**

- Add `features` to `AppState`, defaulting to all on everywhere tests construct `AppState`. Prefer adding it once to a shared test
  constructor if one exists.
- Wrap each group's `.merge(...)` lines in a conditional.
- `fubbik serve` calls `Config::load()` and fails startup with the `ConfigError` message on error.

- [ ] **Step 6: Regenerate the OpenAPI document and client types**

Use the repo's existing generation command: find it in `package.json` / `Justfile` / `crates/fubbik/src/main.rs` (`openapi`
subcommand). Then run `cargo test -p fubbik-api --test openapi` and expect a pass.

- [ ] **Step 7: Write `fubbik.example.toml`**

Include a `[features]` table listing all seven keys set to `true`, with one comment line each saying what the group covers and what it
requires.

- [ ] **Step 8: Verify, then commit**

Run: `cargo test -p fubbik-config && cargo clippy -p fubbik-config -p fubbik-api --all-targets -- -D warnings && just rust-test-fast`
Expected: pass. Then commit `feat(config): add fubbik.toml feature switches and /api/capabilities`.

---

### Task 2: Web nav follows `/api/capabilities`

**Files:**
- Modify: `apps/web/src/features/nav/navigation-items.ts`, plus the nav components that read it (find them with
  `grep -rn "navigation-items" apps/web/src`)
- Create: `apps/web/src/features/nav/use-capabilities.ts`
- Test: next to the existing web unit tests (look for the repo's vitest config in `apps/web`), as `use-capabilities.test.ts`

**Interfaces:**
- Consumes: `GET /api/capabilities` → `{ features: string[] }` from Task 1, through the generated client in `packages/client` (use the
  same client the rest of `apps/web` uses).
- Produces:
  - An optional `feature?: "plans" | "requirements" | "matrices" | "coordination" | "learning_paths" | "overlays" | "ai"` field on each
    nav item.
  - `filterNavItems<T extends { feature?: string }>(items: T[], enabled: Set<string> | null): T[]`. A `null` set means every item is
    kept.
  - `useCapabilities(): Set<string> | null`, which is `null` while loading or on error.

- [ ] **Step 1: Tag each nav item with its group**

Use the table in the plan header:
- `/plans` → `plans`
- `/requirements` and `/coverage` → `requirements`
- `/matrices` → `matrices`
- `/learn` → `learning_paths`
- `/features` → `overlays`

Leave the rest untagged.

- [ ] **Step 2: Write the failing tests**

- `keeps_untagged_items`
- `hides_items_whose_feature_is_disabled`
- `nav_falls_back_to_all_items_when_capabilities_fail` (`null` → all items)

- [ ] **Step 3: Implement `filterNavItems` and `useCapabilities`**

Use the app's existing query library (TanStack Query, if that's what `apps/web` uses). Apply the filter where the nav renders, for both
desktop and `mobile-nav.tsx`.

- [ ] **Step 4: Verify, then commit**

Run the web unit tests and `pnpm run check-types`, and expect both to pass. Then commit
`feat(web): hide nav items for disabled features`.

---

### Task 3: Move personal settings out of the code

**Files:**
- Modify:
  - `crates/fubbik-db/src/seed/spaces.rs:8-20`
  - `apps/web/vite.config.ts:23`
  - `docker-compose.selfhost.yml` (only if `git grep` shows a personal hostname or path there)
  - `.env.example`
  - `CLAUDE.md` (the "Local Development with Caddy" section, and the VS Code test command using `/Users/pontus/...`)
- Create: `docs/guide/operations/local-https.md`

**Interfaces:**
- Produces these env vars, all documented in `.env.example` with a commented-out example:
  - `FUBBIK_SEED_SPACE_PATH`: the seed code space's local path. Defaults to the canonicalised current directory.
  - `FUBBIK_SEED_REMOTE_URL`: the seed space's remote. Defaults to the output of `git remote get-url origin` in the current directory,
    or none if that fails.
  - `FUBBIK_WEB_ALLOWED_HOSTS`: comma-separated extra Vite `server.allowedHosts`. Defaults to none, since localhost is always allowed
    by Vite. Read it with Vite's `loadEnv` with `envDir` set to the repo root.

- [ ] **Step 1: List every personal reference**

Run: `git grep -nE '/Users/pontus|fubbik\.test|Pontusg45' -- ':!legacy' ':!docs/superpowers' ':!pnpm-lock.yaml'`

Classify each hit:
- **personal setting:** fix it in this task
- **test fixture string** (e.g. `crates/fubbik/src/main.rs:483` `parse_cors_origins` input, or `crates/fubbik-cli/src/commands/open.rs:68`
  test URLs): switch to `example.test`, which is neutral
- **seed catalog content:** Task 5

- [ ] **Step 2: Write the failing seed test**

Add the test where `spaces.rs` has or can have unit tests. `seed_space_uses_env_overrides`: given both env vars set, the values used
are the env values. Factor the value resolution into a pure function
`fn seed_space_location(env_path: Option<String>, env_remote: Option<String>, cwd: &Path, git_remote: impl Fn(&Path) -> Option<String>) -> (Vec<String>, Option<String>)`
so the test needs no database.

- [ ] **Step 3: Implement the seed change, the Vite change and `.env.example`**

- [ ] **Step 4: Move the Caddy section to `docs/guide/operations/local-https.md`**

Make it a neutral recipe that uses `app.<your-domain>.test`. Explain that `FUBBIK_WEB_ALLOWED_HOSTS` and `CORS_ORIGIN` carry the
hostnames. In CLAUDE.md, replace the section with one line linking the doc, and make the VS Code command
`code --extensionDevelopmentPath="$PWD/apps/vscode" "$PWD"`.

- [ ] **Step 5: Verify, then commit**

Run the Step 1 grep again. The only remaining hits should be in Task 5's seed catalog files. Then run `just rust-test-fast` and
`pnpm run check-types`, and expect both to pass. Commit `chore(config): move personal hostnames and paths into .env`.

---

### Task 4: Retire the TypeScript CLI and MCP server, and make `legacy/` removable

**Files:**
- Move: `apps/cli` → `legacy/cli`, `packages/mcp` → `legacy/mcp`
- Modify:
  - `package.json` (`workspaces`), `pnpm-workspace.yaml` (`packages`), `knip.json`, `turbo.json` (if either names them)
  - `docker/build/server.Dockerfile:12,19` and `docker/build/web.Dockerfile:20,27` (drop the `COPY` lines)
  - `.gitignore:78` (`apps/cli/fubbik.config.json` → `legacy/cli/fubbik.config.json`)
  - `legacy/migration/rust-migration.json` (the `apps/cli/...` and `packages/mcp/...` inventory roots → `legacy/cli/...` and
    `legacy/mcp/...`)
  - Doc-comment citations in `crates/` and `packages/client/src/api-types.ts` / `openapi.json` that name `apps/cli/` or `packages/mcp/`
    (regenerate the two generated files rather than editing them by hand)
  - `crates/fubbik-mcp/src/lib.rs` (`tool_catalog_matches_the_migration_contract`), `scripts/test/test-rust.sh`, `legacy/README.md`,
    `.github/workflows/ci.yml`
  - `pnpm-lock.yaml`, via `pnpm install`

**Interfaces:**
- Produces a CI job `legacy-removable` in `.github/workflows/ci.yml` with these steps:
  1. checkout
  2. `rm -rf legacy`
  3. install the Rust toolchain and pnpm, the same way the existing jobs do
  4. `pnpm install --frozen-lockfile`
  5. `SQLX_OFFLINE=true cargo check --workspace --all-targets`
  6. `pnpm run check-types`

- [ ] **Step 1: `git mv` both directories, then remove them from the workspace and Docker files**

Then run `pnpm install` to update the lockfile. Only the two importers should disappear from it. Check with
`git diff --stat pnpm-lock.yaml`.

- [ ] **Step 2: Make the `legacy/` consumers optional**

- In `crates/fubbik-mcp/src/lib.rs`, change `include_str!("../../../legacy/migration/rust-migration.json")` to read the file at runtime
  with `std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../legacy/migration/rust-migration.json"))`. When the file is
  missing, print `skipping: legacy/migration removed` with `eprintln!` and return.
- In `scripts/test/test-rust.sh`, wrap the two `legacy/migration/scripts/check-rust-migration*.mjs` lines in
  `if [ -d legacy/migration ]; then … fi`.
- In `legacy/README.md`, rewrite the dependency paragraph: the MCP test and `test-rust.sh` now skip when the folder is absent, and only
  the two `package.json` scripts (`check:rust-migration`, `test:rust-migration`) need removing by a fork.

- [ ] **Step 3: Update the moved paths**

Update the manifest roots and the remaining references. Then run `node --test legacy/migration/scripts/check-rust-migration.test.mjs`
and `node legacy/migration/scripts/check-rust-migration.mjs`, and expect both to pass.

- [ ] **Step 4: Add the `legacy-removable` CI job**

Copy the toolchain setup from the existing `ci.yml` jobs.

- [ ] **Step 5: Simulate the CI job locally**

Do this in a throwaway copy, never in the worktree. Run `git worktree add <scratchpad>/legacy-check HEAD`, then `rm -rf legacy` inside
it, then the job's last three commands. Expect all to pass. Then remove the throwaway worktree.

- [ ] **Step 6: Verify, then commit**

Run `just rust-test-fast` and `pnpm run check-types` in the real worktree, and expect both to pass. Then commit
`chore(repo): retire TypeScript CLI and MCP to legacy/ and make legacy/ removable`.

---

### Task 5: Refresh the seed catalog's moved paths and document the starter

**Files:**
- Modify: `crates/fubbik-db/src/seed/chunks-catalog.json`, `crates/fubbik-db/src/seed/file-links-catalog.json`,
  `crates/fubbik-cli/src/commands/seed_conventions.rs`, and the `packages/db` seed copies only if they're still used by the live seed
  adapter (check `packages/db/src/seed`)
- Modify: `README.md` (Quickstart)

**What changes:** paths, and the factual sentences around them, that name `apps/server`, `packages/api`, `packages/auth`, `apps/cli` or
`packages/mcp`. Each points at the live Rust location where one exists (for example `packages/api/src/chunks` → `crates/fubbik-api/src/chunks`,
the MCP server → `crates/fubbik-mcp`, the CLI → `crates/fubbik-cli`). Otherwise it points at `legacy/...`. Glob patterns
(`appliesTo`, `file-links`) must match real files at HEAD. Check each changed glob with `git ls-files '<glob>' | head -1`.

- [ ] **Step 1: List the stale references**

Run: `git grep -nE 'apps/server|packages/api|packages/auth|apps/cli|packages/mcp' -- crates/fubbik-db/src/seed crates/fubbik-cli/src/commands/seed_conventions.rs`

- [ ] **Step 2: Rewrite each reference**

Keep the JSON valid, and don't change chunk IDs or titles.

- [ ] **Step 3: Run the seed tests**

Run: `just rust-test` (it starts the PG18 test container). It must pass, including `crates/fubbik-db/tests/seed*`. If Docker isn't
available, report BLOCKED with the error rather than skipping.

- [ ] **Step 4: Document the starter**

In README.md's Quickstart, say that `pnpm seed` loads the fubbik demo, and that `cargo run -p fubbik -- seed --scenario minimal` loads a
neutral starter for forks.

- [ ] **Step 5: Commit**

Commit `chore(seed): point demo knowledge at live paths`.

---

### Task 6: Review-harness rule file and `init`

**Files:**
- Create: `crates/review-harness/src/rules.rs`, `crates/review-harness/review-rules.example.toml`
- Modify: `crates/review-harness/src/{lib.rs,main.rs,checks/mod.rs}`, `crates/review-harness/Cargo.toml` (add `toml` and `serde`
  workspace deps, plus `regex = "1"`)
- Create: `review-rules.toml` at the repo root, holding fubbik's own three starter rules

**Interfaces:**
- Produces:
  - `pub struct PatternRule { name: String, description: String, pattern: regex::Regex, roots: Vec<String>, extensions: Vec<String>, message: String, allow: Vec<String> /* repo-relative glob-free path prefixes exempt from the rule */ }`,
    which implements `Check`.
  - `pub fn load_rules(path: &Path) -> anyhow::Result<Vec<PatternRule>>`
  - In the TOML, each rule is a `[[rule]]` table with those field names. `pattern` is a regex.
- CLI changes:
  - `review-harness run` also loads `<root>/review-rules.toml` when present. A `--rules <path>` flag overrides that path.
  - `review-harness init [--root .]` writes `review-rules.toml` from the embedded example, and refuses with
    `review-rules.toml already exists` if the file is present.
  - `list` shows the file rules as well.
- Rule-file errors name the rule and the field, for example `rule "no-as-child": invalid pattern: …`.

- [ ] **Step 1: Write the failing tests in `rules.rs`**

- `loads_rules_from_toml`
- `reports_line_numbers_for_matches`
- `allow_prefix_exempts_files`
- `invalid_regex_names_the_rule`
- `init_refuses_to_overwrite`: run the binary through `env!("CARGO_BIN_EXE_review-harness")` against a temp dir

- [ ] **Step 2: Implement**

Rule matching is line by line, reusing `source::collect` for the file walk.

- [ ] **Step 3: Write the repo's `review-rules.toml`**

It holds three rules:
1. `no-as-child`: pattern `\basChild\b`, roots `["apps/web/src"]`, extensions `[".tsx"]`, message "use the base-ui `render` prop, not
   Radix `asChild`".
2. `no-arktype`: pattern `from ["']arktype["']`, roots `["apps", "packages"]`, extensions `[".ts", ".tsx"]`, message "use Elysia `t`;
   arktype was removed". Allow `packages/env/`, since `README.md` says env validation still uses Arktype; check that first.
3. `given-when-then`: skip it. It needs multi-line structure, which a pattern rule can't express. Write it as a comment in the file
   explaining why it's a planned compiled check.

Run `cargo run -q -p review-harness -- run`. It must report zero violations on the current tree. If it reports any, either the rule or
its `allow` list is wrong; fix the rule, not the code.

- [ ] **Step 4: Verify, then commit**

Run `cargo test -p review-harness`, clippy, and `just rust-test-fast`, and expect all to pass. Then commit
`feat(review-harness): load pattern rules from review-rules.toml`.

---

### Task 7: Separate generic and project agent material, and add `fk agents link`

**Files:**
- Move: `agents/skills/fubbik-dev` → `agents/project/skills/fubbik-dev`
- Create: `crates/kit/src/agents.rs`, `crates/kit/tests/agents.rs`
- Modify: `crates/kit/src/{lib.rs,main.rs}`, `agents/README.md`, `.claude/skills/fubbik-dev` (symlink, re-created by the new command)

**Interfaces:**
- Produces `kit::agents::link(repo_root: &Path, dry_run: bool) -> anyhow::Result<Vec<LinkAction>>`, where
  `enum LinkAction { Created { link: PathBuf, target: PathBuf }, AlreadyLinked { link: PathBuf }, Replaced { link: PathBuf, old_target: PathBuf }, Pruned { link: PathBuf } }`
  derives `Serialize`.
  - Sources:
    - each dir in `agents/skills/*` and `agents/project/skills/*` that contains `SKILL.md` gets a link at `.claude/skills/<name>`
    - `agents/commands` gets a link at `.claude/commands`
  - Targets are relative paths.
  - An existing symlink pointing somewhere else is replaced.
  - A symlink in `.claude/skills/` that points into `../../agents/` but whose target no longer exists is pruned.
  - Links into `.agents/` (third-party skills) are never touched.
  - A real file or directory at a link path fails the whole run, before changing anything, with
    `refusing to replace real file <path>`.
- CLI: `fk agents link [--dry-run]`. It works outside git operations, the same way `port kill` does, and finds the repo root with
  `git rev-parse --show-toplevel` (falling back to the current dir).

- [ ] **Step 1: Write the failing tests in `crates/kit/tests/agents.rs`**

Build a temp repo layout for each.
- `link_creates_relative_symlinks`
- `link_is_idempotent`: a second run gives only `AlreadyLinked`
- `link_replaces_a_stale_symlink`
- `link_prunes_dangling_agents_links_but_not_third_party`
- `link_refuses_to_replace_a_real_file`: nothing changes on disk
- `dry_run_changes_nothing`

- [ ] **Step 2: Implement it, then move `fubbik-dev`**

Run `cargo run -q -p kit -- agents link`, then
`find .claude -type l ! -exec test -e {} \; -print`, and expect no output.

- [ ] **Step 3: Update `agents/README.md`**

- `agents/skills/` holds generic, shippable skills.
- `agents/project/` holds material specific to this repository, which a fork replaces.
- `fk agents link` replaces the manual `ln -s` steps.

- [ ] **Step 4: Verify, then commit**

Run `cargo test -p kit`, clippy, and `just rust-test-fast`, and expect all to pass. Then commit
`feat(kit): add fk agents link and split project agent material`.

---

### Task 8: Rewrite CLAUDE.md for the Rust backend

**Files:**
- Modify: `CLAUDE.md` only

**What changes:** replace the sections that describe the retired TypeScript backend as the architecture with accurate Rust-era text.
Base it on the crates as they are; read `crates/*/src/lib.rs` module docs and `Cargo.toml` files.
- **"Tech Stack" Backend and Database:**
  - axum, sqlx with embedded migrations as the schema authority, Postgres 18 with pgvector, pg_trgm and AGE
  - Drizzle remains read-only for Studio
- **"Backend: Repository -> Service -> Route" and "Context Pipeline (Unified)":** describe the Rust crate layering and the Rust context
  pipeline, then drop the "Retired reference" notes.
- **"MCP Server":** `crates/fubbik-mcp`, run by `fubbik mcp`.
- **"CLI Commands":** the Rust `fubbik` binary. Check the list against `cargo run -q -p fubbik -- --help`.
- **New section "Feature switches":** `fubbik.toml` and `/api/capabilities`, from Tasks 1 and 2.

Keep "Core Concepts", the API endpoint lists, the web pages list and every other section, correcting only facts that are now false.

- [ ] **Step 1: Check every command the file claims exists**

For each command named in "CLI Commands" and "Common Commands", confirm it exists via `--help`, `package.json` or the `Justfile`. List
any that don't exist in the report, and remove them.

- [ ] **Step 2: Rewrite the sections listed above**

- [ ] **Step 3: Verify, then commit**

Run: `grep -nE 'Elysia|Effect\.|Drizzle ORM|Eden|Commander|packages/api' CLAUDE.md`
Expected: hits only in sentences that explicitly describe `legacy/`, or in the frontend section where it's still true (check the
frontend uses no Eden). Then commit `docs(claude-md): describe the Rust backend`.

---

### Task 9: Docs for forking, extending, and using the tools without forking

**Files:**
- Create: `crates/kit/README.md`, `crates/review-harness/README.md`, `docs/forking.md`, `docs/extending.md`
- Modify: `README.md` ("Making it yours", toolkit table)

**What each doc contains:**
- **`crates/kit/README.md`:**
  - install with `cargo install --git https://github.com/Pontusg45/fubbik kit`, taking the remote from `git remote get-url origin`
  - every subcommand with its safety rules
  - `--json`
- **`crates/review-harness/README.md`:**
  - install
  - the `review-rules.toml` format
  - `init`
  - the `Check` trait for compiled checks
- **`docs/forking.md`, an ordered checklist:**
  1. fork
  2. copy `.env.example` → `.env`
  3. `fubbik.toml` features
  4. `fubbik seed --scenario minimal`
  5. replace `agents/project/` and run `fk agents link`
  6. edit CLAUDE.md, or generate it with `fubbik sync-claude-md` from tagged chunks (Decision 4)
  7. write `review-rules.toml`
  8. delete `legacy/` plus the two `package.json` migration scripts
  9. rename the project

  Each step names the exact files.
- **`docs/extending.md`, the stable extension points:**
  - stable: the `Check` trait, `review-rules.toml`, `fk` subcommands (and how to add one), `fubbik.toml` feature groups (and how to add a
    group), MCP tools in `crates/fubbik-mcp`, chunk templates, API route modules
  - may change without notice: everything else, with a sentence saying so
- **`README.md`:**
  - add a "Use the tools without forking" section that links the two crate READMEs
  - point "Making it yours" at `docs/forking.md` and `docs/extending.md`
  - mark the review harness as "Available" in the toolkit table if Task 6 landed

- [ ] **Step 1: Write the docs**

Check every command and path against the tree, and list in the report anything you couldn't check.

- [ ] **Step 2: Verify, then commit**

Run: `for p in $(grep -ohE '\`[a-z./_-]+/[a-zA-Z0-9./_-]+\`' docs/forking.md docs/extending.md crates/kit/README.md crates/review-harness/README.md | tr -d '\`' | sort -u); do [ -e "$p" ] || echo "missing: $p"; done`
Expected: only intentional non-paths or fork-time paths, each explained in the report. Then commit
`docs: add forking and extending guides and tool READMEs`.
