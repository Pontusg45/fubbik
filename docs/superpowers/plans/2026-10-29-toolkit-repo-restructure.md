# Toolkit Repo Restructure Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rearrange the repo around the toolkit (the "Making your AI less of an Fubbik" framing) with one home for agent material,
grouped shell scripts, retired TypeScript moved out of the way, and a new Rust CLI `fk` with safe everyday git and dev commands.

**Architecture:** File moves come first, each in its own commit, verified by the existing test and check commands. `fk` is a new crate
in the flat Cargo workspace (`crates/*`). It calls the `git` and `lsof` binaries rather than linking libraries, so its behaviour matches
what the user would see running those commands by hand.

**Tech Stack:** Rust 2024 (workspace deps: `anyhow`, `clap`, `serde`, `serde_json`), shell, Just, pnpm.

**Spec:** The structure proposal agreed in conversation on 2026-10-29. It is summarised in the Decisions section below, because no
separate spec file exists.

## Decisions

These were proposed in conversation and not yet confirmed, so the plan uses the stated default. The user can override any of them before
execution.

1. **Retired TypeScript moves to `legacy/`, not deleted.** Only directories already outside the pnpm workspace move: `apps/server`,
   `packages/api`, `packages/auth`, plus `migration/` and `tests/fixtures/node-contract*`. `apps/cli`, `packages/db` and `packages/mcp` stay
   for now, because they're still pnpm workspace members and Rust tests `include_str!` files from `packages/db`.
2. **The CLI is named `fk`.** It lives in `crates/kit`, Cargo package `kit`, binary `fk`.
3. **`docs/superpowers/` stays where it is.** The superpowers skills write there by default, so merging it into `docs/plans/` would just
   fight the tool.
4. **Agent material:** `agents/` holds what the user writes. `.agents/skills/` stays the install target for third-party skills managed by
   `skills-lock.json`. `.claude/` contains only symlinks into those two plus `settings.local.json`.
5. **The dev env file moves from `apps/server/.env` to the repo-root `.env`** (already gitignored by `.env` on line 12 of `.gitignore`).
   `apps/server/.env.example` moves to the root `.env.example`.
6. **`fk` skeleton scope:** `sync`, `push`, `branch` and `port kill` only. `wip`, `undo`, `tidy`, `wt`, `db`, `run` and `check` are
   follow-ups and not in this plan.

## Global Constraints

- **Never run `git push`.** The repo's CLAUDE.md says to always ask before pushing. `fk push` tests use a local bare repo only.
- **Branch first:** the work happens on `chore/toolkit-restructure`, created from `main`.
- **The working tree is already dirty with unrelated user work.** Stage only the paths each task names. Never use `git add -A` or `git add .`.
  `Cargo.lock` and `README.md` already carry unrelated changes. Commit `Cargo.lock` only if `git diff Cargo.lock` shows nothing beyond
  the new crates' entries. Otherwise leave it unstaged and say so in the task report.
- **Commit messages** use the repo's conventional style (`chore(repo): …`, `feat(kit): …`), with no co-author trailer.
- **Rust test bodies** use `// Given`, `// When`, `// Then` comments (see the README "Test conventions" section).
- **Moves of tracked files** use `git mv` so history follows them.
- **After every task:** `cargo fmt --all --check` passes and `just rust-test-fast` passes.

## Review Focus

1. **`fk sync` on a branch with no upstream** must fail with `no upstream configured for <branch>` and leave HEAD unchanged, not fall
   through to a fetch-only success. Test: `sync_without_upstream_errors` (Task 4).
2. **`fk push` with no terminal** (when an agent or CI runs it) must refuse unless `--yes` is passed, never block waiting on stdin.
   Test: `push_requires_yes_without_tty` (Task 4).
3. **`fk` run outside a git repo** must print `not a git repository: <dir>` instead of git's raw stderr. Test: `errors_outside_repo`
   (Task 4).
4. **`fk port kill` on a free port** must succeed with `port 3000 is free`, matching today's `pnpm kill:server`. It must not be an
   error, because `just dev` calls it on every start. Test: `free_port_is_not_an_error` (Task 5).
5. **Broken symlinks after the agents move** would silently drop skills from Claude Code. Task 1's verification step checks every link in
   `.claude/` resolves.

---

### Task 1: Consolidate agent material under `agents/`

**Files:**
- Move: `.agents/skills/fubbik-dev/` → `agents/skills/fubbik-dev/` (the user's own skill, not in `skills-lock.json`)
- Move: `docs/skills/build-knowledge-system.md` → `agents/skills/build-knowledge-system/SKILL.md`, adding frontmatter
- Move: `.claude/commands/fubbik.md` → `agents/commands/fubbik.md`
- Create: `agents/mcp/claude-code.json`, `agents/README.md`
- Modify: `.claude/skills/fubbik-dev` (re-point the symlink); create symlinks `.claude/skills/build-knowledge-system` and `.claude/commands`

- [ ] **Step 1: Create the branch**

Run: `git switch -c chore/toolkit-restructure`
Expected: `Switched to a new branch 'chore/toolkit-restructure'`

- [ ] **Step 2: Move the files with `git mv`, then add frontmatter to `build-knowledge-system/SKILL.md`**

```yaml
---
name: build-knowledge-system
description: Use when asked to build a full fubbik knowledge base from a codebase — chunks, connections, tags, requirements, matrices and enrichment.
---
```

- [ ] **Step 3: Re-create the symlinks with relative targets**

`.claude/skills/fubbik-dev` → `../../agents/skills/fubbik-dev`, `.claude/skills/build-knowledge-system` →
`../../agents/skills/build-knowledge-system`, and `.claude/commands` → `../agents/commands` (a directory symlink, which replaces the old
directory). The vendored skill symlinks into `.agents/skills/` stay untouched.

- [ ] **Step 4: Write `agents/mcp/claude-code.json`**

Use the `mcpServers` block from `docs/guide/operations/integrations/mcp.md` verbatim, with `FUBBIK_SERVER_URL` set to
`http://localhost:3000`.

- [ ] **Step 5: Write `agents/README.md`**

It explains the split in a short table: `agents/` holds what you write, `.agents/skills/` holds installed skills (managed by
`npx skills` and `skills-lock.json`, never edited by hand), and `.claude/` holds symlinks only. It also explains how to add a skill: create
`agents/skills/<name>/SKILL.md`, then `ln -s ../../agents/skills/<name> .claude/skills/<name>`. Subagents go in `agents/subagents/` and
hooks in `agents/hooks/`, both created the first time something goes in them.

- [ ] **Step 6: Verify every link resolves and the lockfile is untouched**

Run: `find .claude -type l ! -exec test -e {} \; -print` and `git diff --quiet skills-lock.json && echo lock-ok`
Expected: no output from `find`, and then `lock-ok`

- [ ] **Step 7: Commit**

```bash
git add agents .agents/skills/fubbik-dev docs/skills .claude/skills .claude/commands
git commit -m "chore(agents): move authored skills and commands into agents/"
```

---

### Task 2: Group `scripts/` by purpose

**Files:**
- Move to `scripts/test/`: `test-rust.sh`, `rust-test-db.sh`
- Move to `scripts/release/`: `package-release.sh`, `smoke-production-image.sh`, `smoke-release-archive.sh`, `smoke-rust-seed.sh`
- Leave in place: the five parity scripts (`check-rust-migration*.mjs`, `compare-seed-parity.mjs`, `differential.sh`,
  `capture-node-contract.sh`). Task 3 moves them straight to `legacy/migration/scripts/`.
- Modify: `Justfile:67,71,75`, `.github/workflows/{ci,rust,release}.yml`, `CONTRIBUTING.md:79`, `docs/operations/self-hosted.md:64-74`,
  `migration/rust-migration.json` (the `scripts/test-rust.sh` evidence path)

- [ ] **Step 1: `git mv` each script into its folder**

- [ ] **Step 2: Fix the repo-root lookup inside the moved scripts**

Scripts compute the root as one level up from themselves. It's now two:
- In `test-rust.sh`, `package-release.sh` and `smoke-production-image.sh`, change `"$(dirname "${BASH_SOURCE[0]}")/.."` to `/../..`.
- Inside `test-rust.sh` and `smoke-rust-seed.sh`, the calls `./scripts/rust-test-db.sh` become `./scripts/test/rust-test-db.sh`.

- [ ] **Step 3: Update every caller listed under Files**

- [ ] **Step 4: Verify no old path remains**

Run: `grep -rnE 'scripts/(test-rust|rust-test-db|package-release|smoke-)' --exclude-dir=node_modules --exclude-dir=target --exclude-dir=.git --exclude-dir=superpowers . | grep -vE 'scripts/(test|release)/'`
Expected: no output

- [ ] **Step 5: Run the moved checks**

Run: `node --test scripts/check-rust-migration.test.mjs && node scripts/check-rust-migration.mjs && just rust-test-fast`
Expected: all pass

- [ ] **Step 6: Commit**

Stage the moved scripts and each modified file by name, then run `git commit -m "chore(scripts): group scripts into test and release"`.

---

### Task 3: Move retired TypeScript and migration material to `legacy/`

**Files:**
- Move: `apps/server` → `legacy/server`, `packages/api` → `legacy/api`, `packages/auth` → `legacy/auth`, `migration/` →
  `legacy/migration/`, `tests/fixtures/node-contract*` → `legacy/migration/fixtures/`, and the five parity scripts in `scripts/` →
  `legacy/migration/scripts/`
- Move: `apps/server/.env.example` → `.env.example`
- Modify: `crates/fubbik-mcp/src/lib.rs:1121` (`include_str!` path), `legacy/migration/rust-migration.json` (the `packages/api/` root and
  every `migration/` or `scripts/check-rust-migration…` path), `legacy/migration/scripts/*` (repo-root depth is now three levels),
  `scripts/test/test-rust.sh:42-43`, `package.json:46-47`, `legacy/migration/{README,seed-parity,http-differential}.md`, `Justfile:31`, `docker-compose.yml:43`, `CONTRIBUTING.md:15`, and the
  `tests/fixtures/node-contract` doc comments across `crates/` (about 30 files, found by grep)
- Create: `legacy/README.md`

- [ ] **Step 1: Move the user's real env file (it's untracked, so no `git mv`)**

Run: `test -e .env && echo "STOP: root .env exists" || mv apps/server/.env .env`
Expected: no output. If it prints STOP, ask the user before going on, because a root `.env` would be overwritten.

- [ ] **Step 2: `git mv` every directory and file listed under Files**

- [ ] **Step 3: Update paths**

- `Justfile:31` changes to `source .env`.
- `docker-compose.yml:43` changes to `- .env`.
- `CONTRIBUTING.md:15` changes to "Copy `.env.example` to `.env`".
- `include_str!` changes to `"../../../legacy/migration/rust-migration.json"`.
- In the migration scripts, the repo root becomes `"../../.."` in the `.mjs` files and `/../../..` in the shell scripts. That means
  `check-rust-migration.mjs:7` and any `$SCRIPT_DIR/..` in `capture-node-contract.sh` and `differential.sh`. The manifest's default path
  becomes `legacy/migration/rust-migration.json`. In `check-rust-migration.test.mjs:51,63,75`, the path becomes `"../rust-migration.json"`.
- In `scripts/test/test-rust.sh:42-43` and `package.json:46-47`, the paths become `legacy/migration/scripts/check-rust-migration*.mjs`.
- Rewrite every `tests/fixtures/node-contract` to `legacy/migration/fixtures/node-contract` using `sed` over the files that
  `grep -rl 'tests/fixtures/node-contract' crates` returns.
- `rmdir tests/fixtures tests` if they're empty afterwards.

- [ ] **Step 4: Write `legacy/README.md`**

Three short paragraphs:
1. This folder holds the retired Node/Bun backend and the Node → Rust migration evidence. Nothing in the running product depends on it.
2. `crates/fubbik-mcp` embeds `legacy/migration/rust-migration.json` in one test, and `scripts/test/test-rust.sh` runs the parity checks.
   Remove both before deleting this folder.
3. Forks can delete the folder once they've done that.

- [ ] **Step 5: Verify nothing still points at the old locations**

Run: `grep -rnE 'apps/server|packages/api/|packages/auth|"migration/|tests/fixtures/node-contract|scripts/(check-rust-migration|compare-seed-parity|differential|capture-node-contract)' --exclude-dir=node_modules --exclude-dir=target --exclude-dir=.git --exclude-dir=legacy --exclude-dir=docs --exclude=pnpm-lock.yaml --exclude=CLAUDE.md --exclude=README.md .`
Expected: no output. CLAUDE.md and README.md are updated in Task 6, and `docs/` keeps its historical plans as written.

- [ ] **Step 6: Run everything that reads the moved files**

Run: `node --test legacy/migration/scripts/check-rust-migration.test.mjs && node legacy/migration/scripts/check-rust-migration.mjs && SQLX_OFFLINE=true cargo test -p fubbik-mcp && just rust-test-fast && pnpm run check-types`
Expected: all pass. `pnpm run check-types` proves no live package imported the moved TypeScript.

- [ ] **Step 7: Commit**

Stage the moved paths, `.env.example` and each modified file by name. Never stage `.env`. Then run
`git commit -m "chore(repo): move retired TypeScript and migration evidence to legacy/"`.

---

### Task 4: `fk` crate with safe git commands

**Files:**
- Create: `crates/kit/Cargo.toml`, `crates/kit/src/{lib.rs,main.rs,git.rs,sync.rs,push.rs,branch.rs}`, `crates/kit/tests/git.rs`
- Modify: `Justfile:82` (add `-p kit` to `rust-test-fast`)

**Interfaces:**
- Produces:
  - `pub struct Git { dir: PathBuf }`, with:
    - `Git::open(dir: impl Into<PathBuf>) -> anyhow::Result<Git>`, which errors with `not a git repository: <dir>`
    - `Git::run(&self, args: &[&str]) -> anyhow::Result<String>` (trimmed stdout; on failure, the error carries git's stderr)
    - `current_branch() -> Result<String>` (errors on detached HEAD)
    - `is_clean() -> Result<bool>`
    - `upstream() -> Result<Option<String>>`
  - `pub struct Commit { pub sha: String, pub subject: String }` (derive `Serialize`)
  - `sync::sync(git: &Git, dry_run: bool) -> Result<SyncOutcome>`, where `enum SyncOutcome { UpToDate, FastForwarded { commits: usize }, WouldFastForward { commits: usize } }`
  - `push::plan(git: &Git) -> Result<PushPlan>`, where `struct PushPlan { branch: String, upstream: Option<String>, commits: Vec<Commit> }`
  - `push::execute(git: &Git, plan: &PushPlan) -> Result<()>`
  - `branch::create(git: &Git, name: &str, base: Option<&str>) -> Result<()>`
- The `fk` CLI: a global `--json` flag, and subcommands `sync [--dry-run]`, `push [--yes]`, `branch <name> [--from <base>]`. Exit code 1 on
  any error.

- [ ] **Step 1: Write the test helper and failing tests in `crates/kit/tests/git.rs`**

The helper is `fn fixture(name: &str) -> (PathBuf /*origin bare*/, PathBuf /*clone*/)`. It runs `git init --bare -b main` for the
origin, then clones it into a temp directory named `kit-<name>-<pid>` and makes one commit there. It sets `user.name`/`user.email` in the
clone, then pushes `main` to the **local bare origin only**. Tests:

| Test                                     | Given → Then                                                                                                   |
| ---------------------------------------- | -------------------------------------------------------------------------------------------------------------- |
| `sync_fast_forwards_when_behind`         | a second clone pushes 2 commits to origin → `FastForwarded { commits: 2 }`, and HEAD equals `origin/main`       |
| `sync_dry_run_does_not_move_head`        | same setup, with `dry_run = true` → `WouldFastForward { commits: 2 }`, HEAD unchanged                            |
| `sync_refuses_when_diverged`             | one local and one remote commit → error contains `diverged`, HEAD unchanged, local commit still present          |
| `sync_without_upstream_errors`           | a new local branch with no upstream → error is exactly `no upstream configured for <branch>`                     |
| `push_plan_lists_unpushed_oldest_first`  | two local commits "a", "b" → `commits` subjects are `["a", "b"]` and `upstream == Some("origin/main")`           |
| `push_plan_without_upstream`             | a new branch with 1 commit → `upstream == None`, and `commits.len() == 1` (counted against `--remotes`)          |
| `branch_refuses_dirty_tree`              | an untracked or modified file → error contains `uncommitted changes`, and the branch is not created              |
| `branch_creates_from_base`               | `create(git, "feat/x", Some("main"))` → `current_branch() == "feat/x"`                                         |
| `errors_outside_repo`                    | `Git::open(<empty temp dir>)` → error is exactly `not a git repository: <dir>`                                 |
| `push_requires_yes_without_tty`          | run the `fk push` binary (via `env!("CARGO_BIN_EXE_fk")`) with stdin from `/dev/null` and no `--yes` → exit 1, stderr contains `--yes`, and origin is unchanged |

- [ ] **Step 2: Run the tests to check they fail**

Run: `cargo test -p kit`
Expected: compile errors for the missing `kit` items

- [ ] **Step 3: Write `Cargo.toml`, modelled on `crates/review-harness/Cargo.toml`**

Package `kit`, with `[[bin]] name = "fk"`. Dependencies are `anyhow`, `clap`, `serde` and `serde_json`, all with `.workspace = true`.

- [ ] **Step 4: Implement `git.rs`, `sync.rs`, `push.rs` and `branch.rs`**

- **`Git::open`** runs `git -C <dir> rev-parse --git-dir`.
- **`sync`**:
  1. Require an upstream.
  2. Run `fetch --prune`.
  3. Count with `rev-list --left-right --count HEAD...@{u}`. If the left count is above 0 and the right count is above 0, the branches
     have diverged. Fail with `<branch> has diverged from <upstream>; refusing to rebase — resolve it by hand`.
  4. If only the right count is above 0, run `merge --ff-only @{u}`, unless it's a dry run.
  5. Never call `pull` or `rebase`.
- **`push::plan`** uses `log --reverse --format=%H%x09%s HEAD --not --remotes`.
- **`push::execute`** runs `push` if there's an upstream, otherwise `push -u origin <branch>`.
- **`branch::create`** checks `is_clean`, then runs `switch -c <name> [<base>]`.

- [ ] **Step 5: Implement `main.rs`**

- `push` prints the plan: `Push N commit(s) from <branch> to <upstream or "origin (new branch)">:`, then one `  <short sha> <subject>`
  line per commit.
- Without `--yes`, it prompts `Push? [y/N] ` only when stdin is a terminal (`std::io::IsTerminal`). With no terminal, it errors with
  `refusing to push without a terminal; pass --yes`.
- With `--json`, it prints the `PushPlan` or `SyncOutcome` as JSON instead of text.

- [ ] **Step 6: Run the tests to check they pass, then clippy**

Run: `cargo test -p kit && cargo clippy -p kit --all-targets -- -D warnings`
Expected: 10 passed, no warnings

- [ ] **Step 7: Add `-p kit` to `rust-test-fast` (`Justfile:82`), then run `just rust-test-fast`**

Expected: pass

- [ ] **Step 8: Commit**

```bash
git add crates/kit Justfile
git commit -m "feat(kit): add fk with safe sync, push and branch"
```

Then apply the `Cargo.lock` rule from Global Constraints.

---

### Task 5: `fk port kill`, and use it for dev port cleanup

**Files:**
- Create: `crates/kit/src/port.rs`, `crates/kit/tests/port.rs`
- Modify: `crates/kit/src/{lib.rs,main.rs}`, `Justfile:20-22`, `package.json:57-59`

**Interfaces:**
- Consumes: the CLI layout from Task 4
- Produces: `port::pids(port: u16) -> Result<Vec<u32>>` (`lsof -ti tcp:<port>`; when lsof exits 1 with empty output, that means no
  process and returns `[]`), and `port::kill(port: u16, dry_run: bool) -> Result<KillOutcome>`, where
  `enum KillOutcome { Free, Killed { pids: Vec<u32> }, WouldKill { pids: Vec<u32> } }`
- CLI: `fk port kill <port>... [--dry-run]`

- [ ] **Step 1: Write the failing tests**

- `free_port_is_not_an_error`: bind nothing to a port picked with `TcpListener::bind("127.0.0.1:0")` and then dropped. Expect
  `kill(port, false)` to return `Free`, and the CLI to print `port <n> is free` and exit 0.
- `kills_listener_on_port`: spawn `python3 -c "import socket,time; s=socket.socket(); s.bind(('127.0.0.1', <port>)); s.listen(); time.sleep(30)"`
  and poll `pids(port)` until it's non-empty (2s timeout). Then `kill(port, false)` returns `Killed` with the child's pid, and
  `child.wait()` reports that it was killed.
- Every test in this file returns early, passing, when `lsof` isn't on `PATH`. In that case `pids` errors with
  `lsof not found; fk port needs lsof`.
- `dry_run_keeps_listener_alive`: same setup with `dry_run`. Expect `WouldKill`, and the child is still running.

- [ ] **Step 2: Run the tests to check they fail**

Run: `cargo test -p kit port`
Expected: FAIL, because `port` isn't defined

- [ ] **Step 3: Implement `port.rs` and the subcommand**

Send SIGKILL with `kill -9 <pid>`, the same as the current recipes.

- [ ] **Step 4: Run the tests to check they pass**

Run: `cargo test -p kit && cargo clippy -p kit --all-targets -- -D warnings`
Expected: pass

- [ ] **Step 5: Point dev cleanup at `fk`**

- `Justfile` `kill-dev` becomes `cargo run -q -p kit -- port kill 3000 3001`.
- `package.json` `kill:server` becomes `cargo run -q -p kit -- port kill 3000`, `kill:web` becomes `… 3001`, and `kill:all` becomes
  `… 3000 3001`.

- [ ] **Step 6: Verify**

Run: `just kill-dev`
Expected: `port 3000 is free` / `port 3001 is free` (or `killed …`), exit 0

- [ ] **Step 7: Commit**

```bash
git add crates/kit Justfile package.json
git commit -m "feat(kit): add fk port kill and use it for dev port cleanup"
```

---

### Task 6: Update README, CLAUDE.md and CONTRIBUTING to match

**Files:**
- Modify: `README.md` (the "Project Structure" tree, the "Making it yours" table, the toolkit table, the Getting Started `.env` path),
  `CLAUDE.md` (the "Project Structure" tree and the `.env` path), `CONTRIBUTING.md`

- [ ] **Step 1: Replace both Project Structure trees with the new layout**

`crates/` lists `review-harness` and `kit`. Add `agents/`, the `scripts/{test,release}` folders and `legacy/`. Remove `apps/server`,
`packages/api` and `packages/auth`.

- [ ] **Step 2: README updates**

- Add a "⚡ `fk` terminal CLI | Skeleton | Yes. Standalone Rust binary" row to the toolkit table.
- Add a short `fk` section listing `sync`, `push`, `branch` and `port kill`, and state the safety rules: fast-forward only, push asks
  first.
- In "Making it yours", point "AI instructions" at `CLAUDE.md` and `agents/`.
- Add a row saying `legacy/` is safe to delete after reading `legacy/README.md`.

- [ ] **Step 3: Verify that no doc refers to moved paths as live**

Run: `grep -nE 'apps/server|packages/api|packages/auth|scripts/(test-rust|rust-test-db|package-release)' README.md CLAUDE.md CONTRIBUTING.md`
Expected: no output, except deliberate mentions inside a sentence about `legacy/`

- [ ] **Step 4: Commit**

Stage `README.md` only if its other pre-existing changes are the user's and they've agreed to commit them. Otherwise stage `CLAUDE.md`
and `CONTRIBUTING.md` only and report that `README.md` is left unstaged. Then run
`git commit -m "docs: describe the toolkit layout, fk and legacy/"`.
