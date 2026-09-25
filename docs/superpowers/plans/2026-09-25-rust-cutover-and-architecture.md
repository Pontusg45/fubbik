# Rust Runtime Cutover and Architecture Plan

**Goal:** Finish the runtime migration so production and operational workflows run without Node or Bun, remove the legacy backend and CLI packages, and then deepen the Rust CLI modules without changing their public behavior.

**Architecture:** The shipped artifact becomes one Rust binary plus PostgreSQL. The frontend remains TypeScript at build time but is emitted as a client-side application and served by the Rust HTTP process. Database seeding becomes a transactional Rust maintenance command. Wire contracts live in a dependency-light Rust crate shared by the API, CLI, and MCP implementations. The CLI retains one public entry point while transport, domain operations, discovery, and rendering become private modules behind small interfaces.

**Delivery rule:** Complete the runtime cutover tasks before structural refactors. Each task ends with focused tests and its own commit. Do not combine behavior changes with file moves.

## Current State

- Backend route inventory: 52 classified, none pending.
- CLI command inventory: 63 classified, none pending.
- MCP source inventory: 12 classified, none pending.
- Production server image contains only the Rust binary.
- The seed image and seed Compose command still require Bun.
- The web runner and its runtime URL rewriting still require Bun.
- Legacy backend, CLI, database, authentication, and MCP packages remain in the workspace.
- The migration checker validates source classification but does not prove the cutover gates.
- `fubbik-cli/src/client.rs` is a 1,400-line interface with many untyped JSON results.
- CLI commands repeat output-mode branching and direct terminal writes.

## Global Constraints

- Preserve existing HTTP, CLI, JSON, quiet-output, MCP, and browser behavior unless a task explicitly changes a documented contract.
- Keep Node/pnpm available in builder and frontend-test stages. They must be absent from production runtime images.
- Every new test body must contain Given/When/Then comments.
- Use the existing PostgreSQL 18 test adapter from `scripts/rust-test-db.sh` for database suites.
- Regenerate and diff `openapi.json` whenever public HTTP types or routes change.
- Run `cargo fmt --all` and `cargo clippy --workspace --all-targets -- -D warnings` before every commit.
- Use explicit `git add` pathspecs. Do not push.
- Preserve legacy code only until its replacement passes differential and production smoke tests.

## Target Runtime

```text
Browser
   │
   ▼
fubbik Rust process
   ├── /api/*       Axum API
   ├── /assets/*    immutable frontend assets
   ├── /*            SPA history fallback
   ├── CLI           HTTP client and maintenance commands
   └── MCP           stdio server
          │
          ▼
      PostgreSQL 18
```

The frontend builder may use Node and pnpm. The final application and seed images contain neither Node nor Bun.

---

## Task 0: Make Migration Completion Truthful

**Files:**

- Modify: `migration/rust-migration.json`
- Modify: `scripts/check-rust-migration.mjs`
- Modify: `scripts/check-rust-migration.test.mjs`
- Modify: `migration/README.md`

**Interface:** `checkMigration()` reports inventory parity and each cutover gate separately. It only reports the migration as complete when every gate is verified.

- [ ] Replace string-only `cutoverGates` with objects containing `id`, `status`, `evidence`, and `verification`.
- [ ] Add explicit gates for Rust seed parity, Rust-served web assets, runtime image inspection, legacy-package removal, HTTP differential tests, CLI compatibility, MCP contracts, and release smoke testing.
- [ ] Change the success message to distinguish `inventory complete` from `cutover complete`.
- [ ] Reject a gate marked complete when its evidence path is absent.
- [ ] Add fixture tests for incomplete, invalid, and fully verified gate sets.
- [ ] Mark the existing inventory gates complete and runtime/cleanup gates pending.

**Validation:**

```bash
node --test scripts/check-rust-migration.test.mjs
node scripts/check-rust-migration.mjs
```

**Commit:** `fix(migration): verify runtime cutover gates`

---

## Task 1: Establish a Rust Seed Module and Command

**Files:**

- Create: `crates/fubbik-db/src/seed/mod.rs`
- Create: `crates/fubbik-db/src/seed/{context,planner,fixtures,verify}.rs`
- Create: `crates/fubbik-db/src/seed/modules/*.rs`
- Modify: `crates/fubbik-db/src/lib.rs`
- Modify: `crates/fubbik/src/main.rs`
- Add tests under: `crates/fubbik-db/tests/seed.rs`

**Interface:**

```rust
pub struct SeedOptions {
    pub scenario: SeedScenario,
    pub only: BTreeSet<SeedModule>,
    pub skip: BTreeSet<SeedModule>,
    pub reset: ResetPolicy,
}

pub async fn seed(pool: &PgPool, options: &SeedOptions) -> Result<SeedReport>;
```

The binary exposes:

```text
fubbik seed [--scenario minimal|demo|extended]
            [--only module,module]
            [--skip module,module]
            [--reset auto|none]
            [--quiet]
```

- [ ] Port the module dependency planner first as pure Rust.
- [ ] Test unknown modules, dependency closure, skip conflicts, deterministic ordering, and cycle detection.
- [ ] Port the named-reference fixture model without hard-coded generated IDs.
- [ ] Port `core` and `self-documenting`, establishing the transaction, shared context, user bootstrap, and verification pattern.
- [ ] Port spaces, tags, chunks, connections, and file links.
- [ ] Port use cases, requirements, plans, and agent coordination.
- [ ] Port documents, vocabulary, workspaces, collections, and behavioral matrices.
- [ ] Execute reset modules in reverse dependency order.
- [ ] Run the entire seed in one transaction so any module failure rolls back all changes.
- [ ] Return a structured report containing selected modules, inserted counts, verified counts, and failures.
- [ ] Add database tests for all scenarios, repeated execution, `--reset=none`, `--only`, `--skip`, FK integrity, and rollback.
- [ ] Compare Rust and TypeScript seed row counts and stable named relationships on fresh databases.

**Validation:**

```bash
./scripts/rust-test-db.sh start
DATABASE_URL="$(./scripts/rust-test-db.sh url)" cargo test -p fubbik-db --test seed
DATABASE_URL="$(./scripts/rust-test-db.sh url)" cargo run -- seed --scenario minimal
```

**Commit sequence:**

1. `feat(db): add seed planning and transaction framework`
2. `feat(db): port core knowledge seed modules`
3. `feat(db): port planning and coordination seed modules`
4. `feat(db): complete rust seed scenarios and verification`

---

## Task 2: Replace the Bun Seed Image

**Files:**

- Rewrite: `docker/build/seed.Dockerfile`
- Modify: `docker-compose.selfhost.yml`
- Modify: `package.json`
- Add: `scripts/smoke-rust-seed.sh`

- [ ] Build the `fubbik` binary in the seed image with the same pinned Rust toolchain as the server image.
- [ ] Use a minimal Debian runner containing only the binary, CA certificates, and required shared libraries.
- [ ] Change the Compose command to `fubbik seed --quiet`.
- [ ] Preserve `SEED_DATABASE=false` as a successful no-op at the Compose level.
- [ ] Replace the root `seed` script with the Rust command or a clearly named legacy-only compatibility script until deletion.
- [ ] Add a smoke test that starts a fresh database, runs the seed image, verifies representative rows, runs it again, and verifies idempotence.
- [ ] Assert `node`, `npm`, `pnpm`, and `bun` are absent from the seed runner.

**Commit:** `build(seed): run database seeding in rust`

---

## Task 3: Produce a Static Frontend Build

**Files:**

- Modify: `apps/web/vite.config.ts`
- Modify: `apps/web/src/lib/api-origin.ts`
- Modify frontend route/bootstrap files required by TanStack Router
- Delete after cutover: `apps/web/entrypoint.sh`
- Add or adapt browser tests under `apps/web/e2e/`

**Decision:** Production becomes a client-side application. API requests use same-origin `/api` paths. Development may still proxy `/api` to the configured Rust server. This removes SSR and runtime bundle rewriting from the shipped application.

- [ ] Inventory route loaders and server-only imports that currently rely on the TanStack Start server runtime.
- [ ] Move required initial data fetching to client loaders using the existing generated client.
- [ ] Replace runtime `SSR_API_ORIGIN` and bundle text rewriting with same-origin API URLs.
- [ ] Configure the production build to emit static HTML, JavaScript, CSS, fonts, and images only.
- [ ] Verify direct navigation and browser refresh on representative nested routes.
- [ ] Verify authentication redirects, error pages, graph routes, and asset URLs with a non-root deployment origin.
- [ ] Remove server-bundle dependencies from the deployed frontend artifact.

**Validation:**

```bash
pnpm --filter web run check-types
pnpm --filter web run build
pnpm --filter web run test:components
pnpm run test:e2e
```

**Commit:** `refactor(web): emit a static production application`

---

## Task 4: Serve the Frontend from Rust

**Files:**

- Create: `crates/fubbik-api/src/assets.rs`
- Modify: `crates/fubbik-api/src/lib.rs`
- Modify: `crates/fubbik-api/Cargo.toml`
- Modify: `crates/fubbik/src/main.rs`
- Add: `crates/fubbik-api/tests/assets.rs`
- Modify: `docker/build/server.Dockerfile`
- Modify: `docker-compose.selfhost.yml`

**Interface:** `assets::router(root)` serves files and SPA fallback while preserving `/api/*` 404 behavior.

- [ ] Decide whether assets are embedded in the binary or copied beside it. Prefer copied assets if embedding materially increases incremental link time.
- [ ] Serve fingerprinted assets with long-lived immutable caching.
- [ ] Serve `index.html` with no-cache headers.
- [ ] Apply history fallback only to browser routes accepting HTML.
- [ ] Ensure unknown `/api/*` paths remain JSON 404 responses.
- [ ] Add safe path normalization and traversal tests.
- [ ] Add conditional request support with ETag or last-modified headers.
- [ ] Copy the static frontend output into the final Rust server image.
- [ ] Remove the separate Bun web runner and web health check.
- [ ] Route both the web application and API through one published port.

**Validation:**

```bash
cargo test -p fubbik-api --test assets
docker compose -f docker-compose.selfhost.yml up --build -d
curl --fail http://localhost:4717/api/health
curl --fail -H 'Accept: text/html' http://localhost:4717/graph
```

**Commit:** `feat(server): serve the static web application`

---

## Task 5: Add Production Runtime Smoke Tests

**Files:**

- Create: `scripts/smoke-production-image.sh`
- Modify: `.github/workflows/rust.yml`
- Modify: `migration/rust-migration.json`

- [ ] Build the production server and seed images from a clean checkout.
- [ ] Inspect both runner images and fail if Node, npm, pnpm, Bun, TypeScript sources, or legacy runtime package directories exist.
- [ ] Start PostgreSQL, run migrations through server startup, and execute the Rust seed.
- [ ] Verify API health, static assets, SPA fallback, a nested browser route, one authenticated development request, one CLI request, and graceful shutdown.
- [ ] Capture container logs on failure.
- [ ] Run the smoke test in CI after Rust workspace tests.
- [ ] Mark the seed, web runtime, and production-image gates complete only when CI passes.

**Commit:** `test(runtime): prove node-free production images`

---

## Task 6: Introduce Typed Shared Wire Contracts

**Files:**

- Create: `crates/fubbik-contracts/Cargo.toml`
- Create domain modules under: `crates/fubbik-contracts/src/`
- Modify: workspace `Cargo.toml`
- Modify affected DTOs in `crates/fubbik-api/src/`
- Modify: `crates/fubbik-cli/src/client/`
- Modify: `crates/fubbik-mcp/src/`

**Interface:** The contracts crate contains serde request and response types only. It has no Axum, reqwest, sqlx, filesystem, or process dependencies.

- [ ] Start with matrices and context snapshots because the CLI currently decodes much of those responses through `serde_json::Value`.
- [ ] Move stable enums, pagination envelopes, mutation messages, and public resource representations into domain modules.
- [ ] Keep database rows private to `fubbik-db`; map them into public contract types in the API implementation.
- [ ] Make API handlers serialize contract types and CLI/MCP deserialize the same types.
- [ ] Preserve unknown extension fields only where forward compatibility requires them.
- [ ] Reduce untyped JSON results domain by domain: chunks, spaces/tags, requirements, plans/tasks, documents, context, matrices, health/stats.
- [ ] Regenerate OpenAPI after each domain and reject unintended schema changes.
- [ ] Add serialization fixtures for nullable fields, empty envelopes, timestamps, and enum values.

**Commit sequence:** one domain per commit, beginning with `refactor(contracts): share matrix wire types`.

---

## Task 7: Deepen the CLI Client Module

**Files:**

- Replace: `crates/fubbik-cli/src/client.rs`
- Create: `crates/fubbik-cli/src/client/{mod,transport,chunks,spaces,context,requirements,plans,documents,matrices}.rs`
- Split: `crates/fubbik-cli/tests/client.rs`

**External interface:** Commands receive one `Client`. Domain behavior and transport details remain private to the client module.

- [ ] Move base URL normalization, cookies, timeouts, request construction, error decoding, empty-success handling, and query serialization into `transport.rs`.
- [ ] Give each domain implementation typed operations with option structs when more than three related arguments are required.
- [ ] Add a reusable pagination iterator/collector so commands cannot accidentally fetch only the first page.
- [ ] Standardize errors with method, path, status, request ID, and decoded server message.
- [ ] Preserve the current command-facing method names initially, then reduce pass-through methods after call sites migrate.
- [ ] Split HTTP contract tests by domain and share a small mock-server helper.
- [ ] Keep transport tests focused on cross-domain invariants rather than repeating every method implementation.

**Acceptance criteria:** `client/mod.rs` presents a compact interface; no command constructs API paths or decodes response envelopes.

**Commit:** `refactor(cli): deepen the typed client module`

---

## Task 8: Extract Setup Discovery and Import Modules

**Files:**

- Replace: `crates/fubbik-cli/src/commands/setup.rs`
- Create: `crates/fubbik-cli/src/setup/{mod,model,discover,documents,metadata,patterns,connections,preview,import}.rs`
- Add tests under: `crates/fubbik-cli/tests/setup/`

**External interfaces:**

```rust
pub fn discover(root: &Path) -> Result<Discovery>;
pub async fn import(client: &Client, space: &Space, discovery: &Discovery) -> ImportReport;
```

- [ ] Move pure discovery behavior without changing output.
- [ ] Keep traversal, ignore rules, path normalization, and depth limits in one place.
- [ ] Represent detectors as data plus focused detection functions instead of one large match.
- [ ] Make discovery deterministic by sorting paths, tags, chunks, tips, and inferred connections.
- [ ] Separate discovery from space creation and terminal prompting.
- [ ] Return import errors in the report instead of printing within the importer.
- [ ] Add fixture-project tests for JavaScript, TypeScript monorepo, backend-only, documentation-only, malformed metadata, ignored paths, and duplicate connections.
- [ ] Add one real-router database test for discovery through import.

**Commit:** `refactor(cli): extract project discovery module`

---

## Task 9: Centralize CLI Rendering

**Files:**

- Expand: `crates/fubbik-cli/src/output.rs`
- Modify command modules under: `crates/fubbik-cli/src/commands/`
- Add output snapshot tests

**Interface:**

```rust
pub struct CommandOutput<T> {
    pub data: T,
    pub human: String,
    pub quiet: Vec<String>,
}

pub fn render<T: Serialize>(mode: OutputMode, output: CommandOutput<T>) -> Result<()>;
```

- [ ] Add helpers for one object, lists, mutation results, tables, counts, and empty states.
- [ ] Move JSON pretty-printing, quiet identifiers, colors, and terminal writes into the renderer.
- [ ] Convert repeated CRUD commands first, then plans, requirements, matrices, and setup.
- [ ] Keep `watch` and other indefinite commands on a separate streaming interface.
- [ ] Ensure JSON mode never emits human progress text to stdout.
- [ ] Add golden tests for human, JSON, and quiet output of representative commands.

**Commit:** `refactor(cli): centralize command output rendering`

---

## Task 10: Add a Real CLI-to-Router Test Harness

**Files:**

- Create: `crates/fubbik/tests/support/mod.rs`
- Create focused integration suites under: `crates/fubbik/tests/flows/`
- Modify test-only exports in `fubbik-api` if necessary

**Interface:** `TestApplication::start(pool)` provides a real ephemeral HTTP listener, configured CLI client, seeded session state, and shutdown guard.

- [ ] Start the actual Axum router on an ephemeral port.
- [ ] Use `#[sqlx::test]` databases and real migrations.
- [ ] Add full flows for setup/import, matrix lifecycle, requirements and plans, context export, document import, and cleanup.
- [ ] Assert database state after CLI operations, not only response output.
- [ ] Keep wiremock tests for transport failures and malformed external responses.
- [ ] Avoid testing every flag twice; process-level tests cover orchestration, client tests cover exact request contracts.

**Commit:** `test(cli): exercise workflows against the rust api`

---

## Task 11: Make Local Rust Verification One Command

**Files:**

- Modify: `justfile`
- Modify: `scripts/rust-test-db.sh`
- Modify: `README.md`, `CONTRIBUTING.md`, and migration documentation

- [ ] Add `just test-rust` that starts the database adapter, runs formatting, strict Clippy, migration checks, and `cargo test --workspace --no-fail-fast`, then always stops the adapter.
- [ ] Add `just test-rust-fast` for database-free library and CLI contract tests.
- [ ] Print the exact setup command when a developer invokes a database suite without `DATABASE_URL`.
- [ ] Report aggregate passed, failed, and ignored totals.
- [ ] Keep CI on the same script or commands so local and CI behavior cannot drift.

**Commit:** `chore(test): provide one-command rust verification`

---

## Task 12: Delete Legacy Runtime Packages

**Files:**

- Delete: `apps/cli`
- Delete: `apps/server`
- Delete runtime portions of `packages/api`, `packages/auth`, `packages/db`, and `packages/mcp`
- Modify: `package.json`, `pnpm-workspace.yaml`, `turbo.json`, lockfile, Dockerfiles, Compose files, CI, documentation, and editor tasks
- Preserve explicitly named compatibility fixtures under a neutral fixture directory

- [ ] Scan the frontend and VS Code extension for imports from every legacy package.
- [ ] Move genuinely shared frontend-only types before deleting packages.
- [ ] Run HTTP differential tests and archive their final compatibility fixtures.
- [ ] Remove legacy workspace entries and scripts.
- [ ] Remove obsolete Node database, server, CLI, and MCP dependencies from the lockfile.
- [ ] Remove deployment, documentation, service, and development references to legacy commands.
- [ ] Change the migration inventory to verify absence rather than classify retained legacy sources.
- [ ] Run the full Rust, frontend, component, E2E, container, and image-inspection suites.
- [ ] Mark the final cutover gate complete.

**Commit:** `chore(migration): remove legacy runtime packages`

---

## Task 13: Release and Operational Hardening

**Files:** release workflow, installation documentation, completion packaging, container metadata

- [ ] Build versioned binaries for supported Linux and macOS targets.
- [ ] Package shell completions and checksums.
- [ ] Add schema migration compatibility tests across the previous released database version.
- [ ] Exercise SIGTERM shutdown while requests and background jobs are active.
- [ ] Add container metadata, non-root checks, read-only-root compatibility where practical, and a health-check contract.
- [ ] Document backup, restore, seed, upgrade, rollback, and log collection procedures.
- [ ] Publish a release candidate and run the production smoke test against its exact artifacts.

**Commit:** `build(release): package the rust-only application`

---

## Final Verification

```bash
just test-rust
pnpm install --frozen-lockfile
pnpm run check-types
pnpm run lint
pnpm run fmt:check
pnpm --filter web run test:components
pnpm run test:e2e
./scripts/smoke-production-image.sh
```

The work is complete when:

- migration inventory and every cutover gate pass;
- production and seed runner images contain no Node or Bun executable;
- the Rust process serves both `/api/*` and the browser application;
- all operational database workflows use Rust;
- legacy runtime packages are absent from workspace and deployment files;
- typed contracts replace routine untyped JSON handling;
- setup discovery and CLI rendering have small, stable interfaces;
- the full database-backed workspace and browser suites pass from a clean checkout.
