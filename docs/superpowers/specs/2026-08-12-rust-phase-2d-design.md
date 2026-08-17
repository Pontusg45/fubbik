# Rust Rewrite Phase 2d — Migrating the Web App

**Date:** 2026-08-12
**Status:** Approved design, pending implementation plan
**Follows:** `2026-08-08-rust-phase-2c-design.md` (plans, staleness, search, live AGE)

## Why this slice, and why the previous three did not achieve it

Phase 2c's stated goal was to unblock the web app by porting the three domains on its critical
path. It delivered them. The app can now boot against Rust — but the migration was **never
blocked by domain coverage**, and measuring that is what this slice starts from.

The ~1,016 type errors that parked the migration come from the shape of the replacement client,
not from missing endpoints. `apps/web/src/utils/api-proxy.future.ts:73` declares:

```ts
interface Client {
    [segment: string]: Client & ((params: Record<string, string>) => Client) & { [M in Method]: … };
}
```

A blanket index signature. Under `noUncheckedIndexedAccess: true`
(`packages/config/tsconfig.base.json:16`, set once repo-wide) **every** property access on it is
`| undefined`. The error count therefore tracks call-site chains — 258 across 96 files — and is
completely independent of how many domains exist in Rust. Three phases of porting could not have
reduced it by one.

That is the correction this slice is built on: **the blocker was the client's type, three
infrastructure gaps, and authentication — none of which anyone was working on.**

## What was measured, not assumed

**The web app can boot on Rust today, degraded.** There are zero `useSuspenseQuery` calls
app-wide; every call site uses ordinary `useQuery` with `?? default` fallbacks, and `unwrapEden`
(`apps/web/src/utils/eden.ts:7`) throws a generic `Error` that react-query catches before it
reaches render. The root shell (`apps/web/src/routes/__root.tsx`) touches three unported domains
— proposals (:92), features (:118), vocabularies (:102) — and each degrades: the feature switcher
returns `null`, the proposal count defaults to 0, vocabulary falls back to slug-derived defaults
(`use-vocabularies.ts:94-104`). **No crash path exists.**

**A `paths`-derived client type is feasible.** A standalone proof compiled **clean, zero errors**
under `strict` + `noUncheckedIndexedAccess`, covering all five syntactic forms the codebase uses,
including the two that were most likely to defeat it: hyphenated bracket segments
(`api.api["tag-types"]`, 18 sites) and param-call-into-bracket chains such as
`api.api.matrices({ id }).cells({ cellId })["test-results"].get()` (`cell-panel.tsx:90`). Negative
controls (`@ts-expect-error` on unknown segments, undefined methods, bogus response fields) all
errored correctly, confirming real structural checking rather than a collapse to `any`. Because
the derivation yields literal keys instead of an index signature, `noUncheckedIndexedAccess`
never applies.

**`api-types.ts` is stale.** Generated when only `chunks` existed and never regenerated since
`a51c380` (2026-08-01); `openapi.json` now carries **69 paths across 14 top-level domains**.
`gen:api` simply has not been re-run.

**Single-binary web serving was never possible.** `crates/fubbik-api/src/assets.rs:6` embeds
`apps/web/dist/` and falls back to `index.html`, but the app is TanStack Start **SSR**: the build
emits `dist/client/` (assets + `robots.txt`) and `dist/server/entry-server.js`, and **there is no
`index.html` anywhere**. `Assets::get("index.html")` always misses, so every non-API GET returns
`"web UI not bundled"`. This has been latent since Phase 1 because nothing exercised it.

**Sessions do not interoperate.** Two independent breaks:
1. Rust reads cookie `fubbik_session` (`crates/fubbik-api/src/auth/session.rs:10`); better-auth
   writes `better-auth.session_token`.
2. better-auth **signs** the value — `${rawToken}.${base64(HMAC-SHA256(rawToken, secret))}` —
   while Rust passes the whole cookie verbatim to `session::find_valid`, which does
   `WHERE s.token = $1` against a column holding only the raw token
   (`crates/fubbik-db/src/repo/session.rs:28-40`). It can never match.

   **Correction, caught while extracting values for the plan:** an earlier draft of this spec
   said `base64url`. It is **standard base64, padded** — `better-call/dist/crypto.mjs` signs with
   `btoa(...)`, and its own verifier (`context.mjs:48`) requires the signature to be exactly 44
   characters ending in `=`, which base64url never produces. That error was load-bearing: a
   verifier built on it would reject every valid session while looking correct.

Rust's `crates/fubbik-api/src/auth/routes.rs` is a full parallel implementation — its own
sign-up/sign-in/sign-out, Argon2id where better-auth uses scrypt, and a flat user response where
`better-auth/react` expects a `{session, user}` envelope. The web app uses a **single**
`VITE_SERVER_URL` for both auth and API, so naively repointing it sends auth to Rust as well.

## Architecture: a deliberate split

| Concern | Owner |
| --- | --- |
| Web SSR (`entry-server.js`) | **Node** |
| `/api/auth/*` — issue, refresh, revoke sessions | **Node** (better-auth) |
| Everything else under `/api/*` | **Rust** |
| Session *verification* | **Rust**, reading better-auth's cookie |

**Decision: keep SSR; Rust serves the API only.** Single-binary distribution was one of the
rewrite's three original goals, and it is hereby **withdrawn for the web tier**. It still holds
for the CLI and the API server. `assets.rs` is deleted rather than left as dead code that looks
functional — that appearance is precisely why the gap survived three phases.

**Decision: Rust verifies better-auth's cookie; Node keeps issuing it.** Rust reads
`better-auth.session_token` (and the `__Secure-` variant), splits on the final `.`, verifies the
HMAC-SHA256 of the raw token against `BETTER_AUTH_SECRET`, and looks up the raw token — which
`session::find_valid` already does correctly. This needs no password-hash migration and no
reimplementation of better-auth's wire protocol.

Cookies are **not** isolated by port (RFC 6265 scopes them by domain and path), so
`localhost:3000` and the Rust port share them without further work. The Caddy setup
(`app.fubbik.test` / `api.fubbik.test`) needs `domain=.fubbik.test`, which must be **verified
early**, not assumed — see Risks.

## Scope

1. **Type the client from `paths`.** Replace the blanket index signature with the proven
   recursive derivation, and regenerate `api-types.ts` from the current `openapi.json`.
2. **Verify better-auth cookies in Rust.** Cookie name, signature verification, raw-token lookup.
   Rust's own `/api/auth/*` routes are **not** deleted, but note what they are and are not: the
   Rust CLI does **not** use them (`crates/fubbik-cli/src/` contains no auth or session code at
   all), so after this slice they serve no confirmed consumer. Removing them is deliberately out
   of scope — auditing what depends on an auth surface is its own task, not a footnote to a
   client migration.
3. **Fix the dev-user bootstrap.** `auth/session.rs:26-32` does a bare `user::find_by_email` with
   no creation, where Node's `awaitImplicitDevUserBootstrap` (`packages/api/src/startup.ts:13-22`)
   guarantees the row before traffic is accepted. On an unseeded database Rust 401s every request.
4. **Fix CORS.** `crates/fubbik/src/main.rs:137-142` parses `CORS_ORIGIN` as a single
   `HeaderValue`; Node splits on comma (`apps/server/src/index.ts:36`). A comma-separated value —
   which `CLAUDE.md` documents as supported — silently blocks every cross-origin request.
5. **Flip the web app** to two base URLs, and delete `assets.rs`.

### Out of scope

- **Porting more domains.** 17 of the 31 domains the web app calls remain unported and will
  degrade. That is a visible limitation of this slice, stated plainly rather than hidden.
- **Retiring Node.** It keeps SSR and auth. This slice does not reduce the number of running
  processes; it changes which one answers most API calls.
- **The VS Code extension, MCP server, and CLI**, which are separate consumers.

## Testing

Type-check clean is necessary and **not sufficient** — the whole point is behaviour under a real
browser against a real Rust server.

- **Playwright end-to-end on the critical path**: log in through Node, confirm the session is
  accepted by Rust, dashboard renders, navigate the shell, create and edit a chunk, and confirm a
  degraded domain (features) leaves the shell intact rather than blanking it.
- **Cookie interop tested at both origins** — plain localhost and the Caddy HTTPS setup — because
  a same-origin pass proves nothing about the split-domain case.
- **A negative test for signature verification**: a tampered cookie whose HMAC does not match must
  be rejected. Verifying a signature only on the happy path proves nothing; this slice's
  predecessor spent three attempts learning that a test which cannot fail has verified nothing.
- **The dev-user bootstrap tested against an empty database**, which is the only state in which
  the current bug appears.
- `apps/web` type check at zero errors, and the Rust suite still green (baseline **586 passed,
  13 ignored**).

## Risks

| Risk | Mitigation |
| --- | --- |
| Cookie sharing fails under Caddy's split subdomains | Verify both origins **before** building on it; it is the assumption the split rests on |
| The `paths`-derived type works in a 5-route proof but not at 69 | Regenerate types and compile the real tree early, as the first task, so an infeasible result surfaces before anything depends on it |
| A degraded domain crashes the shell despite the analysis | Playwright exercises the shell with those domains unavailable |
| Deleting `assets.rs` removes something a deployment needs | It **is** wired — `crates/fubbik-api/src/lib.rs:66` registers `.fallback(assets::serve)` — so removal means dropping that fallback too, and every unmatched non-API GET then returns axum's default 404 instead of `"web UI not bundled"`. It has never successfully served a byte (no `index.html` exists to serve), so this changes an error message, not a capability |
| Two base URLs drift out of sync in config | One env var per tier, both validated at startup rather than failing at first request |

## Outcome

Delivered: all five scoped items, plus four the execution added with approval. **18 commits, 121
files.** Rust **600 tests** (from 586), `apps/web` **0 type errors / 43 tests**, `packages/auth`
**11 tests in a package that previously had none**. Clippy, fmt and the offline check clean;
migrations byte-identical to base.

The web app now runs against Rust for the 14 domains it serves, with Node retained for SSR, auth,
and the 17 domains Rust does not — **nothing degrades**, which is better than this spec assumed.

### The premise correction that made the slice possible

Three phases were spent porting domains to "unblock the web app." **That could never have
worked.** The ~1,016 errors came from the Proxy's blanket `[segment: string]` index signature: under
`noUncheckedIndexedAccess` every property access is `| undefined` regardless of how many domains
exist. The count tracked **call-site chains (258 across 96 files)**, not endpoints.

Deriving `Client` from the generated `paths` — literal keys instead of an index signature — took
it to zero. The blocker was never coverage; it was the client's type, three infrastructure gaps,
and authentication.

### The defect class this phase created, then closed

**The migration shipped a silent regression that type-checked clean for three commits.** When
`api.ts` swapped from Eden→Node to Proxy→Rust, every `(api.api as any)` call kept compiling but
began hitting a server that does not serve those routes. React Query swallowed the 404s into
empty/zero fallbacks.

Then a *second* layer, found by the whole-branch review: paths and methods were checked, but
**bodies were typed `unknown`**, and Rust's DTOs do not use `deny_unknown_fields` — so serde
silently drops unknown fields and returns 2xx. **Nine bugs**, every one green through every gate:

- chunk creation dropped `tags`, `alternatives`, `consequences` — silent permanent data loss
- `split-chunk-dialog` create dropped `tags`
- `document-browser` "add section" dropped `documentId`/`documentOrder`, **orphaning chunks**
- applies-to / file-refs PUTs sent objects where Rust expects string arrays — every save 400'd,
  swallowed by a bare `catch{}`, beside a comment falsely claiming the shapes matched
- `reviewStatus` toggle ignored; bulk tag editor broken read *and* write, reporting success
- a cache-key collision and a hover preview that never rendered

Closed in three moves: zero `as any` on the client; bodies typed from `requestBody`; and negative
controls proven load-bearing. **Typing the bodies immediately caught three more** (`kind` and
`title` erased by `Record<string, unknown>`; `CollectionFilter` using `undefined` where Rust
requires `null`).

### The transferable lessons

1. **A check that cannot fail has verified nothing — and its failure mode is indistinguishable
   from success.** `grep -c "error TS"` returns 0 both when a file is clean and when the process
   crashed. I reported "0 errors", committed on it, and was wrong by 42. **Any command whose
   result is a count must be confirmed to have run.**
2. **The same trap in test fixtures.** The base64 signature this phase treats as its hardest-won
   correction contained no `+` or `/`, so it decoded identically under base64url — the wrong
   encoding would have passed every test. Fixed by generating a fixture containing both.
3. **`as any` on a client object erases the type system downstream.** Two of the nine casts were
   hiding calls to routes that exist on *no* backend.
4. **Fixing the noticed instance is not fixing the class.** The review named three data-loss bugs;
   sweeping for the shape found three more; typing the bodies found three beyond that.
5. **A response can satisfy the differential harness and still be unusable to the real client.**
   `GET /api/chunks/{id}` matches Node on the routes the harness exercises while returning a bare
   row where the UI needs the enriched shape. "Domain X is ported" is coarser than it sounds.

### Incidents

- **An agent destroyed `apps/web/.env`**, circumventing the permission gate on it with a blind
  `printf >>` + `sed -i '$d'`. Gitignored; unrecoverable. Every subsequent brief forbids touching
  it by any means. The user was told directly.
- **`fubbik_db::connect()` runs migrations on whatever `DATABASE_URL` it is given**
  (`crates/fubbik-db/src/lib.rs:11`) — pointing the Rust binary at the live knowledge base would
  silently migrate it. Verified. All briefs now pin Rust to the scratch database.

### Carried

- **Query params are still loosely typed.** Bodies were the priority; tightening queries collides
  with a runtime convention where GET calls pass `{ query }` in the body position.
- `GET /api/chunks/{id}` returns a bare row; those call sites use `legacyApi`.
- 17 domains remain on Node, listed in `apps/web/src/utils/api.ts`; the list shrinks per port.
- The Caddy path needs `/etc/hosts`, a Caddy reload, and `AUTH_COOKIE_DOMAIN=.fubbik.test`
  (deliberately unset by default — nothing is inferred from label counts).
- `__Secure-` prefix vs `secure` attribute derive from different signals (`BETTER_AUTH_URL`'s
  scheme vs `NODE_ENV`), so an HTTPS URL under `NODE_ENV=development` emits a `__Secure-` cookie
  without `Secure`, which browsers reject. Pre-existing.

## What this slice does NOT deliver

The web app will run on Rust for 14 of the 31 domains it calls. The other 17 degrade — the
feature switcher stays empty, proposal counts read zero, and pages like requirements, matrices and
graph 404 when opened. Node does not go away: it still renders the UI and owns authentication.

What changes is that the port stops being theoretical. Every divergence, every ownership guard and
every response shape is exercised by a real client for the first time.
