# Forking fubbik

Fubbik is meant to be forked. This is the ordered checklist for making a fork your own. Each step names the files it touches. To use only
the standalone tools instead, see [Use the tools without forking](../README.md#use-the-tools-without-forking). For what you can safely build
on afterwards, see [extending.md](extending.md).

1. **Fork and clone.** Fork the repository on GitHub, then clone your fork. Needs Postgres 18 with pgvector, Rust, pnpm and bun. Ollama is
   optional.

2. **Create your `.env`.** Copy `.env.example` to `.env` (gitignored) and set `DATABASE_URL` and `BETTER_AUTH_SECRET`. Personal settings such
   as `FUBBIK_SEED_SPACE_PATH`, `FUBBIK_SEED_REMOTE_URL` and `FUBBIK_WEB_ALLOWED_HOSTS` also go here. See
   [Environment Variables](guide/getting-started/configuration/environment.md). For HTTPS dev domains, see
   [Local HTTPS](guide/operations/local-https.md).

3. **Choose your features.** Copy `fubbik.example.toml` to `fubbik.toml` and commit it. Its `[features]` table switches `plans`,
   `requirements`, `matrices`, `coordination`, `learning_paths`, `overlays` and `ai` on or off; all are on by default. `matrices` needs
   `requirements`, and `coordination` needs `plans`. Unknown keys are errors. The server reads `./fubbik.toml`, or the file named by
   `FUBBIK_CONFIG`. A disabled feature's API routes and background jobs are not mounted and the web nav hides it
   (`GET /api/capabilities` lists what is on). The database schema is always complete. `ai = false` only unmounts `/api/ai`; enrichment and
   semantic search stay on and degrade without Ollama.

4. **Load a neutral starter.** Run `cargo run -p fubbik -- seed --scenario minimal` instead of `pnpm seed`, which loads the fubbik demo
   knowledge base. Other scenarios are `demo` (default) and `extended`.

5. **Replace the project agent material.** `agents/project/skills/` holds this repo's own skills (`fubbik-dev`); delete it and add yours. Keep
   or remove the generic `agents/skills/` and `agents/commands/` as you like. Then run `cargo run -p kit -- agents link` (or install `fk`)
   to symlink them into `.claude/skills/` and `.claude/commands`. See [`crates/kit/README.md`](../crates/kit/README.md).

6. **Rewrite `CLAUDE.md`.** Edit it by hand, or generate it from tagged chunks with `fubbik sync` (alias `fubbik sync-claude-md`):
   `fubbik sync --tag <tag> --output CLAUDE.md --dry-run` previews, `--watch` keeps it current. Review the result; the committed `CLAUDE.md`
   describes fubbik itself.

7. **Write your review rules.** `review-rules.toml` at the repo root holds fubbik's own conventions (base-ui `render`, no arktype). Replace
   them with yours; `cargo run -p review-harness -- init --root <dir>` writes a fresh starter. Remember to end directory prefixes in `allow`
   with `/`. See [`crates/review-harness/README.md`](../crates/review-harness/README.md).

8. **Delete `legacy/`.** The retired TypeScript is safe to remove (CI's `legacy-removable` job proves it). Then remove by hand what
   [`legacy/README.md`](../legacy/README.md) lists: the `check:rust-migration` and `test:rust-migration` scripts in the root `package.json`,
   and `pnpm scan` (the `scan` script in the root `package.json` and in `packages/db/package.json`, plus `packages/db/src/scan.ts`). Also
   remove the `legacy-removable` job from `.github/workflows/ci.yml`, since there is nothing left to prove.

9. **Rename the project.** Update the `name` in the root `package.json`, the README title and tagline, the `kit` and `review-harness` install
   URLs (they point at the upstream repo), and the `fubbik` names in `CLAUDE.md`. Renaming the Rust crates and the `fubbik` binary is a
   larger, optional change; nothing requires it.

Finish by running the repo's own checks (`just ci` or `pnpm run ci`) to confirm the fork is green.
