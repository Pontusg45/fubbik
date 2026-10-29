# Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the broken and misleading pieces left after the fork-modularity work, smooth the fork path further, and make development
cheaper on disk and time.

**Architecture:** These are independent fixes, ordered so the build-cache change lands first and every later task benefits from it. Each
task is self-contained and owns its own tests. Docs are updated by the task that changes the behaviour; the final task only updates the
`fubbik-dev` skill and a last cross-doc consistency pass.

**Tech Stack:** Rust (axum, utoipa, clap, sqlx), TanStack Router web app (TypeScript), Just, Docker Compose, pnpm.

**Spec:** The 14 suggestions from the conversation on 2026-10-29, which the user approved with "fix all". This plan covers 12 of them.
Items 7 (`.env.example`; the user's permissions block reading `.env*`) and 14 (committing the user's own work in progress) are the
user's to do and are left out.

## Decisions

1. **The default API port is 3000 everywhere:** the `fubbik serve` default, the CLI and MCP default server URL, the Docker image, the
   Vite proxy, and the docs. Playwright keeps 3100 explicitly, so end-to-end runs don't collide with a running dev server.
2. **The separate web Docker image is removed.** The server image already serves the built web app (`FUBBIK_WEB_ASSETS_DIR`). The dev
   `docker-compose.yml` drops its `web` service and `docker/build/web.Dockerfile` and `apps/web/entrypoint.sh` are deleted.
3. **API docs are served by the Rust server:**
   - `GET /api/openapi.json` returns the generated document. It's always mounted and needs no auth, like `/api/capabilities`.
   - `GET /docs` returns a small HTML page that loads Swagger UI, pinned to an exact version, from `cdn.jsdelivr.net` and points it at
     `/api/openapi.json`. The page also links the raw JSON, so the spec stays readable offline.
   - No new Rust dependency is added: `utoipa-swagger-ui` downloads assets at build time, which fails offline.
4. **Disabled-feature pages are guarded once, in `apps/web/src/router.tsx`,** through the router's `InnerWrap` option. They are not
   guarded per route file: several route files hold the user's uncommitted work. With capabilities unknown, nothing is blocked.
5. **The Node seed moves to `legacy/`.** Once dev compose uses `fubbik seed`, nothing live runs `packages/db/src/seed`. It moves to
   `legacy/db-seed/`, and the `seed:legacy` script goes with it.
6. **`pnpm scan` is removed in favour of `fubbik docs import <dir>`** if that command imports a directory of markdown. If it doesn't,
   Task 7 adds directory support to `fubbik docs import` first; Task 7 decides which applies after checking.
7. **Review-harness baselines:** compiled checks get `fn baseline(&self) -> usize { 0 }`, and a check passes while its count is at most
   its baseline. The Given/When/Then check ships with its baseline set to today's count, so it blocks only new violations.
8. **Shared Rust build cache:**
   - The `Justfile` exports `CARGO_TARGET_DIR` as `<main checkout>/target`, derived from `git rev-parse --git-common-dir`, unless it's
     already set. Every worktree driven by `just` then shares one cache.
   - Dev builds use `debug = "line-tables-only"` for workspace crates and `debug = false` for dependencies.

## Global Constraints

- Work in the worktree `.claude/worktrees/hardening` on branch `chore/hardening`. **Never run `git push`.**
- Stage only the paths a task names, by name. Conventional commit messages, with no co-author trailer. Use `git mv` for moves.
- **The user's main checkout has uncommitted edits.** The list is in the SDD workspace as `user-dirty.txt`. Keep edits to those files
  minimal and local, and never reformat or restructure them.
- Rust test bodies use `// Given`, `// When`, `// Then` comments.
- After every task: `cargo fmt --all --check` and `just rust-test-fast` pass. For TypeScript changes, `pnpm run check-types` and the web
  unit tests pass.
- **The disk is about 99% full.** Run `df -g .` before building. Never run the full `just rust-test` unless the task says so. Never touch
  `.env*` files. Never run `just kill-dev`, `pnpm kill:*`, dev servers, or real port kills.
- Verification greps exclude `node_modules`, `target`, `.git`, `.superpowers`, `docs/superpowers`, `docs/plans` and `legacy`.

## Review Focus

1. **Two worktrees building at once with the shared cache** must not corrupt each other's builds. Cargo's lock serialises them, and
   neither may delete the other's artifacts. No task runs `cargo clean` on the shared cache. Task 1 documents this.
2. **`/docs` with no internet** still gives the user a working link to the raw `/api/openapi.json`. Test: `docs_page_links_raw_spec`
   (Task 4).
3. **Opening a disabled page by URL** shows a "turned off" message and fires none of that page's queries. With capabilities unknown,
   the page renders normally. Tests: `guard_blocks_disabled_route` and `guard_allows_when_capabilities_unknown` (Task 5).
4. **An MCP client calling a disabled tool** gets `feature "<key>" is disabled on this server`, not a raw 404. A server without
   `/api/capabilities` keeps every tool listed. Tests: `disabled_group_tools_are_hidden` and
   `capabilities_unreachable_keeps_all_tools` (Task 6).
5. **The new Given/When/Then check** must not fail the current tree. Its baseline equals today's count: `cargo run -q -p review-harness
   -- run` exits 0 (Task 8).

---

### Task 1: Shared build cache and leaner dev builds

**Files:** `Justfile`, `Cargo.toml` (`[profile.dev]`), `CONTRIBUTING.md` (one paragraph)

- [ ] **Step 1: Export the shared target dir at the top of the `Justfile`**

  `export CARGO_TARGET_DIR := env("CARGO_TARGET_DIR", <absolute git-common-dir>/../target)`, using Just's backtick or `shell()` with
  `git rev-parse --path-format=absolute --git-common-dir`. Every recipe inherits it.

- [ ] **Step 2: Add the dev profile to `Cargo.toml`**

  ```toml
  [profile.dev]
  debug = "line-tables-only"

  [profile.dev.package."*"]
  debug = false
  ```

  Keep any existing `[profile.*]` sections and merge with them.

- [ ] **Step 3: Verify the target dir resolves the same from both checkouts**

  Run: `just --evaluate CARGO_TARGET_DIR` from the worktree, and `cd <main checkout> && just --evaluate CARGO_TARGET_DIR`.
  Expected: the same absolute path both times, ending in `fubbik/target`.

- [ ] **Step 4: Verify a build lands in the shared dir**

  Run `just rust-test-fast` and expect a pass, with artifacts appearing under the shared path. The main checkout's `target/` is absent
  today, so this run creates it.

- [ ] **Step 5: Document it in `CONTRIBUTING.md`**

  - `just` recipes share one cache across worktrees.
  - Plain `cargo` doesn't share it unless `CARGO_TARGET_DIR` is exported the same way.
  - Concurrent builds wait on Cargo's lock.
  - Never `cargo clean` the shared dir while another worktree is building.

- [ ] **Step 6: Commit**

  Commit `chore(build): share one cargo target dir across worktrees and trim dev debug info`.

---

### Task 2: Dev compose uses the Rust seed, drops the broken web image, and the Node seed moves to legacy

**Files:**
- Modify: `docker-compose.yml`
- Delete: `docker/build/web.Dockerfile`, `apps/web/entrypoint.sh`
- Move: `packages/db/src/seed` → `legacy/db-seed`
- Modify: `packages/db/package.json` (drop `seed:legacy` and anything else that imports the moved seed),
  `legacy/migration/scripts/compare-seed-parity.mjs` (new seed path), `legacy/README.md`, `docs/operations/self-hosted.md` (only if it
  mentions the web image), `.github/workflows/*` (only if any builds `web.Dockerfile`)

- [ ] **Step 1: Find every consumer**

  Run: `git grep -nE 'web\.Dockerfile|entrypoint\.sh|src/seed|seed:legacy' -- ':!legacy' ':!docs/superpowers' ':!docs/plans'`
  List each hit with its planned action in the report.

- [ ] **Step 2: Change the compose seed service**

  It runs `fubbik seed --quiet`, the same `sh -c` gate as `docker-compose.selfhost.yml`. Then remove the `web` service and every
  `depends_on` that names it.

- [ ] **Step 3: Delete the web image files and `git mv` the Node seed**

  If something in `packages/db/src` other than the seed imports from `src/seed` (for example a schema-verify script), stop and report
  NEEDS_CONTEXT.

- [ ] **Step 4: Validate the compose file**

  Run: `docker compose -f docker-compose.yml config -q`
  Expected: exit 0. This doesn't build images, so it's safe on disk.

- [ ] **Step 5: Run the remaining checks**

  Run `pnpm run check-types`, then the Step 1 grep again; expect only intended hits. Then run both legacy migration checks:
  `node --test legacy/migration/scripts/check-rust-migration.test.mjs` and `node legacy/migration/scripts/check-rust-migration.mjs`.

- [ ] **Step 6: Commit**

  Commit `chore(docker): seed dev compose with fubbik seed, drop the web image, retire the Node seed`.

---

### Task 3: One default port (3000)

**Files:**
- `crates/fubbik/src/main.rs`: lines 25, 281 and 370. The user has this file dirty, so change those three literals only.
- `crates/fubbik-cli/src/config.rs:121`
- the MCP default URL, wherever `fubbik mcp` falls back (it's `main.rs:281`)
- `docker/build/server.Dockerfile` (`ENV PORT`, `EXPOSE`, `HEALTHCHECK` are already 3000; check)
- `apps/web/vite.config.ts` (`API_PROXY_TARGET` default)
- `agents/mcp/claude-code.json`
- `docs/guide/getting-started/configuration/environment.md`, `CLAUDE.md`, `README.md` (outside the user's CLI section), and
  `docs/guide/operations/integrations/mcp.md`

- [ ] **Step 1: List every port reference**

  Run: `git grep -nE '\b3100\b' -- ':!legacy' ':!docs/superpowers' ':!docs/plans' ':!pnpm-lock.yaml'`
  Classify each hit:
  - **default:** change to 3000
  - **test port:** keep 3100, i.e. Playwright and the e2e support files
  - **unrelated number:** leave

- [ ] **Step 2: Write the failing test**

  In `crates/fubbik-cli`, add `default_server_url_is_port_3000` where `resolve_base_url` already has tests. If none exists, add a unit
  test in `config.rs` with no env or config set.

- [ ] **Step 3: Change the defaults, then run the tests and the grep again**

- [ ] **Step 4: Commit**

  Commit `fix(config): default the API to port 3000 everywhere`.

---

### Task 4: Serve the API docs from the Rust server

**Files:**
- Create: `crates/fubbik-api/src/api_docs.rs`
- Modify: `crates/fubbik-api/src/lib.rs` (mount it, always on), `crates/fubbik-api/src/openapi.rs` (register `/api/openapi.json` if the
  repo registers every route there), `apps/web/vite.config.ts` (keep the `/docs` proxy), `apps/web/src/routes/docs.tsx` (only if its
  URLs are wrong), `openapi.json` and `packages/client/src/api-types.ts` (regenerated with the repo's generator only if the document
  changes)
- Test: `crates/fubbik-api/tests/api_docs.rs`

**Interfaces:**
- `GET /api/openapi.json` returns 200 with `application/json`, and its body equals `serde_json::to_value(ApiDoc::openapi())`.
- `GET /docs` returns 200 with `text/html`. The body loads `swagger-ui-dist@<exact pinned version>` CSS and JS from `cdn.jsdelivr.net`,
  calls `SwaggerUIBundle({ url: "/api/openapi.json", dom_id: "#swagger-ui" })`, and has a visible
  `<a href="/api/openapi.json">Raw OpenAPI JSON</a>`.
- Neither route needs auth.

- [ ] **Step 1: Write the failing tests**

  Follow the harness in `tests/capabilities.rs`:
  - `openapi_json_matches_generated_document`
  - `docs_page_serves_swagger_ui`
  - `docs_page_links_raw_spec`

- [ ] **Step 2: Implement, then run the tests**

  Then run `cargo test -p fubbik-api --test openapi` to check the committed `openapi.json` is still fresh.

- [ ] **Step 3: Update the docs**

  Fix CLAUDE.md's line saying the server doesn't serve Swagger UI.

- [ ] **Step 4: Commit**

  Commit `feat(api): serve /api/openapi.json and a /docs page`.

---

### Task 5: Guard disabled-feature pages, and hide their command-palette entries

**Files:**
- Create: `apps/web/src/features/nav/feature-route-guard.tsx`
- Modify: `apps/web/src/router.tsx` (wire the guard through `InnerWrap`), `apps/web/src/features/nav/use-capabilities.ts` (export a
  `featureForPath(pathname: string): Feature | undefined` if it isn't exported already), and the command-palette file that builds the
  "go to" commands (find it under `apps/web/src/features/command-palette/`; check `user-dirty.txt` first)
- Test: `apps/web/src/features/nav/feature-route-guard.test.tsx` or `.test.ts`

**Interfaces:**
- `FeatureRouteGuard({ children })` reads the current pathname via the router, then calls `featureForPath` and `useFeatureEnabled`.
- When the feature is known to be disabled, it renders a centred message instead of `children`, and the children never mount:
  - heading: `This feature is turned off`
  - text: `"<label>" is disabled in this server's fubbik.toml.`
  - a link to `/dashboard`
- Otherwise it renders `children`.
- The pure decision lives in `shouldBlockRoute(pathname: string, enabled: Set<string> | null): boolean`, so it can be unit-tested.

- [ ] **Step 1: Write the failing tests**

  - `guard_blocks_disabled_route`
  - `guard_allows_when_capabilities_unknown`
  - `guard_allows_core_routes`
  - `guard_blocks_nested_route` (e.g. `/plans/abc`)

- [ ] **Step 2: Wire the guard through `InnerWrap`**

  Implement, then check that route loaders don't fetch for gated routes before the guard renders:
  `grep -n "loader" apps/web/src/routes/{plans,requirements,matrices,learn,features,coverage}*`. If any do, report it and gate that
  loader's fetch on capabilities in the smallest way, without editing a file in `user-dirty.txt`.

- [ ] **Step 3: Filter the palette's "go to" commands with `filterNavItems`, or the same predicate**

- [ ] **Step 4: Verify, then commit**

  Run the web unit tests and `pnpm run check-types`. Then commit `feat(web): show a turned-off page for disabled features`.

---

### Task 6: The MCP server and CLI respect feature switches

**Files:** `crates/fubbik-mcp/src/lib.rs` (tool listing and dispatch), `crates/fubbik-mcp/src/api.rs` (capabilities fetch), the CLI
error path for plan and matrix commands. Prefer the CLI's shared transport error mapping, but `client/transport.rs` is in
`user-dirty.txt`, so if the change there is more than a few lines, put the mapping in `commands/plan.rs` and `commands/matrix.rs`
instead. Tests go in `crates/fubbik-mcp` (existing test style) and `crates/fubbik-cli/tests`.

**Interfaces:**
- **Capabilities cache:** at startup, and lazily on the first `tools/list`, the MCP server fetches `/api/capabilities` once and caches
  `Option<Vec<String>>`. `None` means unknown, so all tools are listed.
- **Tool groups:** each MCP tool group maps to a feature key (plans, requirements, matrices, coordination). Tools of a disabled group
  are left out of `tools/list`. Calling one returns a tool error with the text `feature "<key>" is disabled on this server`.
- **CLI:** when a plan or matrix command gets a 404, the CLI checks `/api/capabilities`. If the group is disabled, it exits 1 with
  `error: feature "<key>" is disabled on this server`. Otherwise it reports the original error unchanged.

- [ ] **Step 1: Write the failing tests**

  - `disabled_group_tools_are_hidden`
  - `calling_disabled_tool_names_the_feature`
  - `capabilities_unreachable_keeps_all_tools`
  - the CLI test `plan_command_reports_disabled_feature`

  Use the mock HTTP setup the crates already use (`wiremock` or the existing test server).

- [ ] **Step 2: Implement, then run the tests**

  Run: `SQLX_OFFLINE=true cargo test -p fubbik-mcp -p fubbik-cli`
  Then run clippy.

- [ ] **Step 3: Update docs**

  Update the "Add a feature group" note in `docs/extending.md`.

- [ ] **Step 4: Commit**

  Commit `feat(mcp,cli): hide and explain tools for disabled features`.

---

### Task 7: Replace `pnpm scan`

**Files:**
- Delete: `packages/db/src/scan.ts`
- Modify: the `scan` script in root `package.json` and `packages/db/package.json`, `packages/db/tsconfig.json` (drop the
  `src/scan.ts` exclude), `legacy/README.md` and `docs/forking.md` (remove scan from the hand-removal list), and `CLAUDE.md` /
  `README.md` if they mention `pnpm scan`
- Possibly modify: the `fubbik docs import` command, per Decision 6. `crates/fubbik-cli/src/lib.rs` is dirty, so keep CLI wiring edits
  minimal.

- [ ] **Step 1: Check what `fubbik docs import` already does**

  Run: `cargo run -q -p fubbik -- docs import --help`, and read the command module. Record whether it accepts a directory and recurses
  `*.md`.

- [ ] **Step 2: Add directory support if it's missing**

  Use TDD with `docs_import_accepts_a_directory`: recurse `*.md`, skip the shared ignore list (`node_modules`, `target`, `.git`), and
  keep the single-file behaviour.

- [ ] **Step 3: Remove scan and update every mention**

  Docs should say: use `fubbik docs import <dir>`.

- [ ] **Step 4: Verify, then commit**

  Run `pnpm run check-types`, `just rust-test-fast`, and the CLI tests. Then commit `chore(db): replace pnpm scan with fubbik docs import`.

---

### Task 8: Review-harness baselines and a Given/When/Then check

**Files:**
- Create: `crates/review-harness/src/checks/given_when_then.rs`
- Modify: `crates/review-harness/src/{lib.rs,runner.rs,checks/mod.rs,main.rs}`, `review-rules.toml` (remove the "planned compiled
  check" comment), `crates/review-harness/README.md`

**Interfaces:**
- `Check::baseline(&self) -> usize`, with a default of 0. `CheckResult` gains `baseline: usize`.
- `Report::passed()` is true when every result has `violations.len() <= baseline`.
- Output prints `[name] N violation(s) (baseline B)` and marks over-baseline checks `FAIL`.
- **The `given-when-then` check:**
  - roots `["crates"]`, extensions `[".rs"]`
  - for each `#[test]` or `#[tokio::test]` function, it finds the body by brace matching and reports the test's line when the body
    lacks any of `// Given`, `// When`, `// Then`. Comments may carry extra text; a combined `// When and Then` counts for both.
  - `legacy/` is excluded by the roots

- [ ] **Step 1: Write the failing tests**

  - `test_with_all_three_markers_passes`
  - `missing_then_is_reported_at_the_test_line`
  - `combined_when_then_counts`
  - `non_test_functions_are_ignored`
  - `braces_inside_strings_do_not_confuse_matching`
  - in the runner: `count_at_baseline_passes` and `count_over_baseline_fails`

- [ ] **Step 2: Implement, then set the baseline**

  Run `cargo run -q -p review-harness -- run` and set the check's baseline to the count it reports, with a comment saying it may only
  go down. Expect exit 0.

- [ ] **Step 3: Verify, then commit**

  Run `cargo test -p review-harness`, clippy, and `just rust-test-fast`. Then commit
  `feat(review-harness): baselines and a Given/When/Then check`.

---

### Task 9: `just test-changed`

**Files:** `Justfile`, `scripts/test/test-changed.sh`, `CONTRIBUTING.md`

**Interfaces:**
- `just test-changed [base]`, where `base` defaults to `main`.
- It runs `git diff --name-only $(git merge-base <base> HEAD)` plus uncommitted changes, maps each changed `crates/<dir>/` to its
  package name (read from that crate's `Cargo.toml` `name`), and adds every workspace crate that depends on a changed one, using
  `cargo metadata --format-version 1 --no-deps`.
- It then runs `cargo test -p` for that set. For sets that include `fubbik-db`, `fubbik-api` or `fubbik`, it starts the DB adapter
  first, the same way `scripts/test/test-rust.sh` does.
- It runs `pnpm run check-types` and the web unit tests when anything under `apps/web` or `packages/` changed.
- With nothing changed, it prints `nothing changed since <base>` and exits 0.

- [ ] **Step 1: Write the script**

  Keep the crate-mapping logic in a function the script can run in a `--dry-run` mode that prints the plan without running anything.

- [ ] **Step 2: Verify with dry runs**

  Run `just test-changed --dry-run` on the branch; it should list the crates this branch changed. Then run it in a throwaway
  `git worktree` at `main` with no changes, under the scratchpad; it should print the nothing-changed message. Remove that worktree
  afterwards.

- [ ] **Step 3: Document it, then commit**

  Document it in `CONTRIBUTING.md`. Then commit `chore(test): add just test-changed`.

---

### Task 10: Refresh the seed catalog's prose for the Rust stack

**Files:** `crates/fubbik-db/src/seed/chunks-catalog.json` (and `core-catalog.json` if it describes the stack)

**What changes:** chunk bodies and summaries that describe the backend as Elysia, Effect, Drizzle, Eden or Better Auth now describe the
live Rust stack: axum, sqlx, `crates/*`, and the session cookie validated in `crates/fubbik-api/src/auth`.
- Chunk IDs, titles, tags and connections stay unchanged, and the JSON stays valid.
- Mentions of the TypeScript stack stay only where a chunk is explicitly about `legacy/`.

- [ ] **Step 1: List the stale passages**

  Run: `grep -noE 'Elysia|Effect|Drizzle|Eden|Better Auth|Commander|Bun' crates/fubbik-db/src/seed/*.json`

- [ ] **Step 2: Rewrite each passage**

  Check every new claim against the crates.

- [ ] **Step 3: Run the seed tests**

  This needs the DB adapter, but not the full suite. Start the DB, then run
  `DATABASE_URL=… FUBBIK_TEST_DB_EXTERNAL=true SQLX_OFFLINE=true cargo test -p fubbik-db --test seed`.

- [ ] **Step 4: Commit**

  Commit `chore(seed): describe the Rust stack in demo knowledge`.

---

### Task 11: Rewrite the `fubbik-dev` skill, and a final docs pass

**Files:** `agents/project/skills/fubbik-dev/SKILL.md` (and any files beside it), plus a consistency pass over `CLAUDE.md`, `README.md`
(outside the user's CLI section), `docs/forking.md`, `docs/extending.md` and `CONTRIBUTING.md`

**What changes:**
- The skill teaches the live Rust patterns, with real file paths for each:
  - the crate layering (`fubbik-db` repo → `fubbik-api` service and routes, `fubbik-contracts` DTOs)
  - how a route module is added and registered
  - utoipa annotations, then regenerating `openapi.json` and `packages/client` types
  - sqlx migrations as the schema authority
  - test patterns: the DB adapter, Given/When/Then, `wiremock` for Ollama
  - feature-group gating
  - the web app's TanStack Router plus the generated client
- Its frontmatter `description` stays a "use when…" sentence.
- Pitfalls come from real ones found this session:
  - `Default::default()` for `features` silently ignores `fubbik.toml`
  - readonly nav arrays
  - `.env*` reading is blocked by permissions
  - the shared target dir
- The docs pass confirms that ports (3000), `/docs`, scan removal, the dev compose, and the web image removal read consistently across
  all the listed docs.

- [ ] **Step 1: Write the skill**

  Verify every path with `ls` or `git grep`.

- [ ] **Step 2: Check the docs for stale statements**

  Run: `git grep -nE '3100|web\.Dockerfile|pnpm scan|entry-server|does not serve Swagger' -- CLAUDE.md README.md docs/forking.md docs/extending.md CONTRIBUTING.md agents`
  Expected: only the Playwright test-port mention.

- [ ] **Step 3: Commit**

  Commit `docs(agents): rewrite fubbik-dev for the Rust codebase`.
