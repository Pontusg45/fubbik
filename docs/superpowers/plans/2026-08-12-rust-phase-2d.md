# Rust Phase 2d Implementation Plan — Migrating the Web App

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Point `apps/web` at the Rust backend for every API call except authentication, keeping Node for SSR and `/api/auth/*`.

**Architecture:** A deliberate split. Node keeps SSR and issues better-auth sessions; Rust answers everything else under `/api/*` and *verifies* better-auth's signed cookie without issuing it. The web app gains a second base URL.

**Tech Stack:** Rust (axum 0.8, sqlx 0.8, `hmac`/`sha2`/`base64`), TypeScript (TanStack Start SSR, `openapi-typescript`), better-auth, Playwright.

## Global Constraints

- **Run cargo in the main tree. Do NOT set `CARGO_TARGET_DIR`. Do NOT create a git worktree** unless told to — disk runs near capacity.
- **`export DATABASE_URL="postgres://postgres:password@localhost:5434/fubbik_rs"`** before running any Rust test, or every sqlx test panics with `DATABASE_URL must be set: EnvVar(NotPresent)` — this looks like real breakage and is pure harness error.
- **Every `docker` command needs `--context orbstack`.** The default `desktop-linux` context points at a Docker Desktop that is not running, and its error misleadingly reads as an API version mismatch. Never run `docker context use` — it rewrites the user's global config.
- **Run tests in the FOREGROUND.** Four agents in the previous slice stalled by backgrounding a run and idling.
- **Commit each task as its tests pass.** Do not hold a large uncommitted draft; two agents in the previous slice lost in-flight work.
- Never edit an applied migration. `crates/fubbik-db/migrations/` must end this slice byte-identical to base.
- **`cargo sqlx prepare --workspace` alone DROPS test-target-only entries.** Use `cargo sqlx prepare --workspace -- --tests`; verify the `.sqlx` diff is additive-only.
- Stale `_sqlx_test_*` databases cause phantom FK failures — drop before investigating any failure:
  `docker --context orbstack exec fubbik-rs-db psql -U postgres -d postgres -Atc "SELECT 'DROP DATABASE IF EXISTS \"'||datname||'\";' FROM pg_database WHERE datname LIKE '\_sqlx\_test%'" | docker --context orbstack exec -i fubbik-rs-db psql -U postgres -d postgres`
- Commit with the explicit pathspec form (`git commit -m "msg" -- <paths>`); `git add` new files first; never `git add -A`. No `Co-Authored-By` or "Generated with Claude" trailers.
- **Do NOT start the Node server against the user's live knowledge base without asking.** Tasks 6 and 7 need it; ask first.
- Rust baseline: **586 passed, 0 failed, 13 ignored**. `apps/web` type check baseline: **0 errors**.

### The cookie contract — exact values, verified against `better-call` 1.3.2

Every one of these was extracted from source and confirmed with a runnable proof. Do not re-derive them; do not "simplify" them.

| Property | Value |
| --- | --- |
| Cookie name | `better-auth.session_token`, or `__Secure-better-auth.session_token` |
| What picks the prefix | **`BETTER_AUTH_URL`'s scheme** — `https://` → `__Secure-`, `http://` → none. **NOT `NODE_ENV`.** |
| Value format | `${rawToken}.${signature}` |
| Signature | **standard base64, padded** — `btoa(HMAC-SHA256(rawToken, secret))`. **NOT base64url.** |
| What is signed | the raw token **alone** — not `name=value` |
| Key | `BETTER_AUTH_SECRET`, UTF-8 bytes used directly as the HMAC key, no KDF |
| Delimiter | split on the **last** `.` (`context.mjs:44` uses `lastIndexOf`) |
| Wire encoding | percent-encoded; `CookieJar` hands back the decoded value |

`better-call`'s own verifier requires the signature to be 44 characters ending in `=` (`context.mjs:48`) — that is how we know it is padded standard base64, not base64url.

**Test vector, produced by running the real `signCookieValue` and re-verified in Rust:**

```
secret    = "test-secret-value-at-least-32-chars-long-000000"
token     = "AbCdEfGhIjKlMnOpQrStUvWxYz012345"
signature = "OyhRBnvMlgxzHjKlrRsQqjXwtDV99cAmrGDWxOTAkzU="
cookie    = "AbCdEfGhIjKlMnOpQrStUvWxYz012345.OyhRBnvMlgxzHjKlrRsQqjXwtDV99cAmrGDWxOTAkzU="
percent-encoded on the wire:
            "AbCdEfGhIjKlMnOpQrStUvWxYz012345.OyhRBnvMlgxzHjKlrRsQqjXwtDV99cAmrGDWxOTAkzU%3D"
```

---

## File Structure

**Create:**
- `crates/fubbik-api/src/auth/better_auth_cookie.rs` — parse and verify better-auth's signed cookie. Standalone and dependency-free apart from `hmac`/`sha2`/`base64`, so it unit-tests without a database.
- `apps/web/src/utils/api-client-types.ts` — the `paths`-derived `Client` type, separate from the runtime Proxy so the type can be reasoned about on its own.
- `apps/web/e2e/critical-path.spec.ts` — Playwright end-to-end.

**Modify:**
- `crates/fubbik-api/src/auth/session.rs` — accept better-auth cookies; create the dev user.
- `crates/fubbik-api/Cargo.toml` — add `hmac`, `sha2`, `base64`.
- `crates/fubbik/src/main.rs` — multi-origin CORS.
- `crates/fubbik-api/src/lib.rs:66` — drop the `assets::serve` fallback.
- `apps/web/src/utils/api-proxy.future.ts` → becomes `api.ts`'s implementation.
- `packages/env/src/web.ts` — add `VITE_API_URL`.
- `apps/web/src/routes/docs.tsx:97,107` — two hardcoded `http://localhost:3000/docs` literals.

**Delete:** `crates/fubbik-api/src/assets.rs`.

---

## Task 1: The `paths`-derived client type

**Files:**
- Create: `apps/web/src/utils/api-client-types.ts`
- Modify: `apps/web/src/utils/api-types.ts` (regenerated), `apps/web/src/utils/api-proxy.future.ts`

**Interfaces:**
- Produces: `export type Client = BuildNode<"">` over the generated `paths`, replacing the blanket index signature.

**This task goes first because it carries the slice's biggest unknown.** A 5-route proof compiled clean; this is 69 paths. If the derivation fails at real scale, everything downstream changes — so it must fail early, not after four tasks have been built on it.

**The defect being fixed:** `api-proxy.future.ts:73` declares `interface Client { [segment: string]: … }`. Under `noUncheckedIndexedAccess: true` (`packages/config/tsconfig.base.json:16`) every property access on that is `| undefined`, producing ~1,016 errors across 96 files **regardless of how many domains exist**. The count tracks call-site chains, not endpoints.

- [ ] **Step 1: Regenerate the API types**

`api-types.ts` was generated when only `chunks` existed and has not been regenerated since `a51c380` (2026-08-01). `openapi.json` now carries **69 paths across 14 top-level domains**.

```bash
cd apps/web && pnpm gen:api
git diff --stat apps/web/src/utils/api-types.ts
```
Expected: a large additive diff. If `gen:api` does not exist, read `apps/web/package.json` for the real script name rather than inventing one.

- [ ] **Step 2: Write the failing type test**

Create `apps/web/src/utils/api-client-types.test-d.ts` with the five call shapes the census found, plus negative controls:

```ts
import type { Client } from "./api-client-types";
declare const api: { api: Client };

// 1. plain chain (140 `.get(` sites)
const _a = api.api.spaces.get();
// 2. param call (91 sites)
const _b = api.api.chunks({ id: "x" }).get();
// 3. literal hyphenated bracket (21 sites)
const _c = api.api["chunk-types"].get();
// 4. param-call into bracket — the deepest real chain, cell-panel.tsx:90
const _d = api.api.matrices({ id: "m" }).cells({ cellId: "c" })["test-results"].get();
// 5. query options (64 sites)
const _e = api.api.chunks.stale.count.get({ query: {} });

// Negative controls — these MUST error, or the type has collapsed to `any`
// @ts-expect-error unknown segment
api.api.chunks.nonexistent.get();
// @ts-expect-error method not defined on this route
api.api.spaces.put();
```

- [ ] **Step 3: Run it to verify it fails**

Run: `cd apps/web && pnpm exec tsgo -p tsconfig.json --noEmit`
Expected: FAIL — `api-client-types` does not exist.

- [ ] **Step 4: Write the derivation**

`Client` is derived recursively from `keyof paths` by template-literal matching, producing **literal keys** rather than an index signature — which is precisely why `noUncheckedIndexedAccess` stops applying. Children whose next segment is `{param}` get a call signature; all others become properties.

```ts
import type { paths } from "./api-types";

type Method = "get" | "post" | "patch" | "put" | "delete";

/** Path keys that continue below `Prefix`, e.g. "/api/chunks" under "/api". */
type ChildRoutes<Prefix extends string> = Extract<keyof paths, `${Prefix}/${string}`>;

/** The single next segment after `Prefix` in `Route`. */
type NextSegment<Prefix extends string, Route extends string> =
    Route extends `${Prefix}/${infer Rest}`
        ? Rest extends `${infer Seg}/${string}` ? Seg : Rest
        : never;

type LiteralSegments<Prefix extends string> =
    Exclude<NextSegment<Prefix, ChildRoutes<Prefix>>, `{${string}}`>;

type ParamSegment<Prefix extends string> =
    Extract<NextSegment<Prefix, ChildRoutes<Prefix>>, `{${string}}`>;

type Methods<Route extends string> = {
    [M in Method as Route extends keyof paths
        ? M extends keyof paths[Route] ? M : never
        : never]: (body?: unknown, options?: { query?: Record<string, unknown> }) => Promise<unknown>;
};

export type BuildNode<Prefix extends string> =
    Methods<Prefix>
    & { [Seg in LiteralSegments<Prefix>]: BuildNode<`${Prefix}/${Seg}`> }
    & ([ParamSegment<Prefix>] extends [never]
        ? unknown
        : (params: Record<string, string>) => BuildNode<`${Prefix}/${ParamSegment<Prefix>}`>);

export type Client = BuildNode<"/api">;
```

Then replace the blanket `interface Client` in `api-proxy.future.ts` with an import of this type. The runtime Proxy is unchanged — only its type changes.

- [ ] **Step 5: Run the type check on the whole web app**

Run: `cd apps/web && pnpm exec tsgo -p tsconfig.json --noEmit`
Expected: PASS, or a **small** number of errors confined to the 9 dynamic-index sites listed below.

**STOP and report if the error count is in the hundreds.** That means the derivation does not hold at 69 paths, and the slice needs re-scoping rather than 900 hand-written guards.

- [ ] **Step 6: Leave the 9 dynamic-index sites as `any`**

These index with a **runtime variable**, which a literal-key type structurally cannot resolve. They already carry `as any` casts and must keep them:

`utils/api-helpers.ts:8,13,18,26` · `features/chunks/use-bulk-chunk-operations.ts:14` ·
`features/search/search-results.tsx:131,172` · `components/smart-link-provider.tsx:212` ·
`routes/search.tsx:192` · `routes/learn.tsx:15` · `routes/learn.$pathId.tsx:19`

Do **not** try to type them. Add a one-line comment at each explaining why, so the next reader does not retry it.

- [ ] **Step 7: Commit**

```bash
git add apps/web/src/utils/api-client-types.ts apps/web/src/utils/api-client-types.test-d.ts
git commit -m "feat(web): derive the API client type from OpenAPI paths" -- apps/web/src/utils/api-client-types.ts apps/web/src/utils/api-client-types.test-d.ts apps/web/src/utils/api-types.ts apps/web/src/utils/api-proxy.future.ts
```

---

## Task 2: Verify better-auth's signed cookie in Rust

**Files:**
- Create: `crates/fubbik-api/src/auth/better_auth_cookie.rs`
- Modify: `crates/fubbik-api/Cargo.toml`, `crates/fubbik-api/src/auth/mod.rs`

**Interfaces:**
- Produces: `pub fn verify(cookie_value: &str, secret: &str) -> Option<String>` — returns the raw token when the signature verifies, `None` otherwise.

Pure logic, no database. Test it in isolation.

- [ ] **Step 1: Add the three crates**

None of `hmac`, `sha2`, `base64` are currently dependencies of `fubbik-api` or `fubbik-db` — only `argon2`, which is unrelated.

```toml
hmac = "0.12"
sha2 = "0.10"
base64 = "0.22"
```

- [ ] **Step 2: Write the failing test, using the verified vector**

```rust
const SECRET: &str = "test-secret-value-at-least-32-chars-long-000000";
const TOKEN: &str = "AbCdEfGhIjKlMnOpQrStUvWxYz012345";
const SIG: &str = "OyhRBnvMlgxzHjKlrRsQqjXwtDV99cAmrGDWxOTAkzU=";

#[test]
fn accepts_a_cookie_signed_by_better_auth() {
    let cookie = format!("{TOKEN}.{SIG}");
    assert_eq!(verify(&cookie, SECRET).as_deref(), Some(TOKEN));
}

#[test]
fn rejects_a_tampered_signature() {
    // Flip one character. A verifier tested only on the happy path proves nothing:
    // the previous slice spent three attempts learning that lesson.
    let bad = format!("{TOKEN}.XyhRBnvMlgxzHjKlrRsQqjXwtDV99cAmrGDWxOTAkzU=");
    assert_eq!(verify(&bad, SECRET), None, "a forged signature must not authenticate");
}

#[test]
fn rejects_a_tampered_token() {
    let bad = format!("BbCdEfGhIjKlMnOpQrStUvWxYz012345.{SIG}");
    assert_eq!(verify(&bad, SECRET), None, "the signature must cover the token");
}

#[test]
fn rejects_a_cookie_with_no_signature() {
    assert_eq!(verify(TOKEN, SECRET), None);
}

#[test]
fn splits_on_the_last_dot_not_the_first() {
    // better-call uses lastIndexOf (context.mjs:44). Tokens are alphanumeric today, so
    // this is unobservable in production — which is exactly why it needs a test: a
    // first-dot split would work until the token alphabet ever changed.
    let cookie = format!("a.b.{SIG}");
    // The signature will not match "a.b", but the SPLIT must still yield "a.b".
    assert_eq!(verify(&cookie, SECRET), None);
    assert_eq!(split_signed(&cookie), Some(("a.b", SIG)));
}
```

- [ ] **Step 3: Run it to verify it fails**

Run: `cargo test -p fubbik-api better_auth_cookie`
Expected: FAIL — module does not exist.

- [ ] **Step 4: Implement**

```rust
use base64::Engine as _;
use hmac::{Hmac, Mac};
use sha2::Sha256;

pub fn split_signed(value: &str) -> Option<(&str, &str)> {
    let idx = value.rfind('.')?;           // LAST dot — better-call uses lastIndexOf
    Some((&value[..idx], &value[idx + 1..]))
}

pub fn verify(cookie_value: &str, secret: &str) -> Option<String> {
    let (token, sig_b64) = split_signed(cookie_value)?;
    // STANDARD base64 with padding, not base64url: better-call signs with btoa()
    // and its own verifier requires 44 chars ending in '='.
    let sig = base64::engine::general_purpose::STANDARD.decode(sig_b64).ok()?;
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).ok()?;
    mac.update(token.as_bytes());
    mac.verify_slice(&sig).ok()?;          // constant-time
    Some(token.to_string())
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p fubbik-api better_auth_cookie`
Expected: PASS, all five.

- [ ] **Step 6: Prove the rejection tests are load-bearing**

Replace `mac.verify_slice(&sig).ok()?` with `let _ = sig;` so every signature "verifies". Confirm `rejects_a_tampered_signature` and `rejects_a_tampered_token` both FAIL. Restore. **Report the messages.** A verifier that cannot reject is not a verifier.

- [ ] **Step 7: Commit**

```bash
git add crates/fubbik-api/src/auth/better_auth_cookie.rs
git commit -m "feat(auth): verify better-auth's HMAC-signed session cookie" -- crates/fubbik-api/src/auth/better_auth_cookie.rs crates/fubbik-api/src/auth/mod.rs crates/fubbik-api/Cargo.toml Cargo.lock
```

---

## Task 3: Accept better-auth sessions in the extractor

**Files:**
- Modify: `crates/fubbik-api/src/auth/session.rs`
- Test: `crates/fubbik-api/tests/auth_session.rs`

**Interfaces:**
- Consumes: `better_auth_cookie::verify` from Task 2.

Today `session.rs:10` reads only `fubbik_session` and passes the value verbatim to `session::find_valid`, which does `WHERE s.token = $1 AND s.expires_at > now()`. Rust's own cookie stays supported; better-auth's is added alongside it.

Both stacks check only `expires_at` for validity, so there is no expiry divergence to reconcile — the break is purely which cookie is read and how its value is decoded.

- [ ] **Step 1: Write the failing test**

```rust
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn a_better_auth_cookie_authenticates(pool: PgPool) {
    let user_id = seed_user(&pool, "a@b.test").await;
    // The session row holds the RAW token; the cookie carries token.signature.
    seed_session(&pool, &user_id, TOKEN).await;
    let app = test_app_with_secret(pool.clone(), SECRET).await;

    let res = get_with_cookie(&app, "/api/chunks",
        &format!("better-auth.session_token={TOKEN}.{SIG}")).await;
    assert_eq!(res.status(), 200, "a validly-signed better-auth cookie must authenticate");
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn a_forged_signature_is_rejected_end_to_end(pool: PgPool) {
    let user_id = seed_user(&pool, "a@b.test").await;
    seed_session(&pool, &user_id, TOKEN).await;
    let app = test_app_with_secret(pool.clone(), SECRET).await;

    let res = get_with_cookie(&app, "/api/chunks",
        &format!("better-auth.session_token={TOKEN}.AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=")).await;
    assert_eq!(res.status(), 401, "an unsigned or forged token must not reach the database");
}

#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn the_secure_prefixed_cookie_name_is_accepted(pool: PgPool) {
    let user_id = seed_user(&pool, "a@b.test").await;
    seed_session(&pool, &user_id, TOKEN).await;
    let app = test_app_with_secret(pool.clone(), SECRET).await;

    let res = get_with_cookie(&app, "/api/chunks",
        &format!("__Secure-better-auth.session_token={TOKEN}.{SIG}")).await;
    assert_eq!(res.status(), 200, "HTTPS deployments get the __Secure- prefix");
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p fubbik-api --test auth_session`
Expected: FAIL with 401 — the cookie name is not read.

- [ ] **Step 3: Implement**

Read `better-auth.session_token`, then `__Secure-better-auth.session_token`, then fall back to Rust's own `fubbik_session`. For the better-auth names, run the value through `better_auth_cookie::verify` and use the returned **raw token** for the lookup. The secret comes from `BETTER_AUTH_SECRET` and belongs in `AppState`, not read per-request.

**Both prefixes must be accepted unconditionally.** The prefix is chosen by `BETTER_AUTH_URL`'s scheme, not by `NODE_ENV`, so a server cannot reliably predict which one the browser will send.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p fubbik-api --test auth_session`
Expected: PASS.

- [ ] **Step 5: Confirm the existing cookie still works**

Run the full suite. Rust's own `/api/auth/*` sign-in flow must be unaffected — its tests are the regression check.

- [ ] **Step 6: Commit**

```bash
git commit -m "feat(auth): accept better-auth session cookies alongside fubbik_session" -- crates/fubbik-api/src/auth/session.rs crates/fubbik-api/tests/auth_session.rs
```

---

## Task 4: Dev-user bootstrap, CORS, and removing `assets.rs`

**Files:**
- Modify: `crates/fubbik-api/src/auth/session.rs`, `crates/fubbik/src/main.rs`, `crates/fubbik-api/src/lib.rs:66`
- Delete: `crates/fubbik-api/src/assets.rs`

Three small infrastructure fixes that share a test cycle.

- [ ] **Step 1: Write the failing tests**

```rust
#[sqlx::test(migrations = "../fubbik-db/migrations")]
async fn implicit_dev_session_creates_the_dev_user_on_an_empty_database(pool: PgPool) {
    // The ONLY state in which the current bug appears. With a seeded DB it passes either way.
    let app = test_app_with_implicit_dev(pool.clone()).await;
    let res = get(&app, "/api/chunks").await;
    assert_eq!(res.status(), 200, "dev session must bootstrap its user, not 401");

    let row = user::find_by_email(&pool, "dev@localhost").await.unwrap();
    assert_eq!(row.unwrap().id, "dev-user");
}

#[test]
fn cors_accepts_a_comma_separated_origin_list() {
    let origins = parse_cors_origins("http://localhost:3001, https://app.fubbik.test:8443");
    assert_eq!(origins.len(), 2, "CLAUDE.md documents comma-separated CORS_ORIGIN as supported");
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p fubbik-api dev_user && cargo test -p fubbik cors`
Expected: FAIL — 401, and no such function.

- [ ] **Step 3: Implement the dev-user bootstrap**

Match Node's `ensureImplicitDevUserRow` exactly (`packages/db/src/repository/implicit-dev-user.ts`): id `"dev-user"`, name `"Dev User"`, email `"dev@localhost"`, `email_verified` false, idempotent via `ON CONFLICT (id) DO NOTHING` followed by a re-select.

Node does this **once at startup, before accepting traffic**. Doing it lazily per-request is acceptable and simpler, but say which you chose in the report — a per-request insert on a hot path deserves a deliberate decision, not a default.

- [ ] **Step 4: Implement multi-origin CORS**

Node splits on `,` and trims (`apps/server/src/index.ts:36`); it does **not** strip trailing slashes, so neither should Rust. Use `AllowOrigin::list(...)`.

**`CorsLayer` with `Any` origins plus `allow_credentials(true)` panics at layer construction** — that was hit in Phase 1 and fixed with `mirror_request()`. `AllowOrigin::list` is credentials-safe.

- [ ] **Step 5: Delete `assets.rs`**

Remove the file and the `.fallback(assets::serve)` registration at `crates/fubbik-api/src/lib.rs:66`.

It **is** wired, so this is not dead-code removal — but it has never served a byte: it embeds `apps/web/dist/`, and the SSR build emits `dist/client/` + `dist/server/` with **no `index.html` anywhere**. After removal, an unmatched non-API GET returns axum's default 404 instead of `"web UI not bundled"`. That changes an error message, not a capability.

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test --workspace`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git commit -m "fix(api): bootstrap the dev user, accept multi-origin CORS, drop dead asset serving" -- crates/fubbik-api/src/auth/session.rs crates/fubbik/src/main.rs crates/fubbik-api/src/lib.rs crates/fubbik-api/src/assets.rs
```

---

## Task 5: Split the web app's base URLs

**Files:**
- Modify: `packages/env/src/web.ts`, `apps/web/src/utils/api.ts`, `apps/web/src/features/import/use-sse-import.ts`, `apps/web/src/routes/docs.tsx`

Auth keeps `VITE_SERVER_URL` (Node). Everything else moves to a new `VITE_API_URL` (Rust).

- [ ] **Step 1: Add the env var**

Add `VITE_API_URL: type("string.url")` to `packages/env/src/web.ts`, mirroring the existing `VITE_SERVER_URL` at lines 7 and 12-13. **Validate both at startup**, so a missing value fails immediately rather than at the first request.

- [ ] **Step 2: Repoint the API consumers**

Three files read `VITE_SERVER_URL` for API traffic and must move to `VITE_API_URL`:
- `apps/web/src/utils/api.ts:5`
- `apps/web/src/utils/api-proxy.future.ts:143`
- `apps/web/src/features/import/use-sse-import.ts:25` — the SSE import stream

`apps/web/src/lib/auth-client.ts:4-6` **stays** on `VITE_SERVER_URL`.

- [ ] **Step 3: Fix the two hardcoded URLs**

`apps/web/src/routes/docs.tsx:97` and `:107` contain the literal `http://localhost:3000/docs` in an `href` and an `iframe src`, ignoring env entirely. Under the split these silently point at the wrong server. Swagger is served by Rust, so both become `${env.VITE_API_URL}/docs`.

- [ ] **Step 4: Verify no other hardcoded backend URL remains**

```bash
grep -rn "localhost:3000\|localhost:3001" apps/web/src --include=*.ts --include=*.tsx
```
Expected: no hits pointing at fubbik's own API. `settings.tsx`'s Ollama placeholder is unrelated and stays.

- [ ] **Step 5: Type-check**

Run: `cd apps/web && pnpm exec tsgo -p tsconfig.json --noEmit`
Expected: PASS, 0 errors.

- [ ] **Step 6: Commit**

```bash
git commit -m "feat(web): split auth and API base URLs" -- packages/env/src/web.ts apps/web/src/utils/api.ts apps/web/src/utils/api-proxy.future.ts apps/web/src/features/import/use-sse-import.ts apps/web/src/routes/docs.tsx
```

---

## Task 6: Cookie interop at both origins

**Files:** none — this is a verification task producing a written finding.

**Needs the Node server and the user's live knowledge base. ASK THE HUMAN PARTNER BEFORE STARTING IT.** Node's startup writes staleness flags there. Issue GETs only; never write to that database yourself.

**This is the assumption the whole split rests on**, and it must be tested before Task 7 builds on it.

- [ ] **Step 1: Ask permission, then start both servers**

Node on 3000 (`pnpm dev:server`), Rust on its own port against the same database.

- [ ] **Step 2: Verify plain-localhost cookie sharing**

Log in through Node, then call a Rust endpoint from the browser with credentials. Cookies are **not** isolated by port (RFC 6265 scopes them by domain and path), so this is expected to work — confirm it rather than assume it.

- [ ] **Step 3: Verify the Caddy HTTPS origins**

`app.fubbik.test:8443` → web, `api.fubbik.test:8443` → API. These are **different subdomains**, so the cookie needs `domain=.fubbik.test` to be sent to both. Check whether better-auth sets a domain attribute today, and what it would take.

**A same-origin pass proves nothing about the split-domain case.** If this fails, report it — do not work around it silently. It changes the deployment story, and the user needs to decide.

- [ ] **Step 4: Stop both servers**

Confirm `lsof -ti:3000` is empty and no stray `turbo`/`bun` processes remain.

- [ ] **Step 5: Record the finding**

Write both results to the report, including exactly which cookie attributes were observed (`Set-Cookie` verbatim) and whether `__Secure-` was used, since `BETTER_AUTH_URL`'s scheme decides that.

---

## Task 7: Playwright end-to-end on the critical path

**Files:**
- Create: `apps/web/e2e/critical-path.spec.ts`
- Modify: `apps/web/package.json`, `apps/web/playwright.config.ts`

**Needs both servers. ASK THE HUMAN PARTNER — and read the next paragraph first, because the permission you need is broader than it looks.**

**Running Playwright *is* starting Node against the user's live knowledge base.** `apps/web/playwright.config.ts:17-24` declares a `webServer` entry `command: "bun run --hot src/index.ts", cwd: "../server", port: 3000`, so `playwright test` launches the Node API server automatically — and Node's startup writes staleness flags to `postgresql://pontus@localhost:5432/fubbik`. Do not treat "I only ran Playwright" as different from "I started Node". Ask for the same explicit permission.

**Playwright is configured but NOT installed.** `playwright.config.ts` and `e2e/auth.spec.ts` both exist and import `@playwright/test`, but the package appears in no `package.json` in the repo and `pnpm exec playwright --version` fails with `Command "playwright" not found`. It is an orphaned setup that must be restored before any of this runs.

Type-check clean is necessary and not sufficient. The point of this slice is behaviour in a real browser against a real Rust server.

- [ ] **Step 0: Restore the toolchain and add the Rust server**

```bash
cd apps/web && pnpm add -D @playwright/test && pnpm exec playwright install chromium
```

Then add a third `webServer` entry for the Rust API alongside the existing Node (3000) and Vite (3001) entries, and set the Vite entry's env so the browser gets `VITE_API_URL` pointing at the Rust port while `VITE_SERVER_URL` stays on 3000 — the split from Task 5.

The Vite entry already sets `VITE_FUBBIK_DISABLE_IMPLICIT_DEV_UX: "true"` (`playwright.config.ts:30-32`), so these tests exercise the **real login path** rather than the implicit dev session. That is what makes step 1 meaningful — keep it.

**Confirm the existing `e2e/auth.spec.ts` passes before writing anything new.** If it does not, say so and stop: a pre-existing broken e2e suite is a finding, and building on it hides whatever is already wrong.

- [ ] **Step 1: Write the spec**

Cover, in one flow:
1. Log in through Node; confirm the session is accepted by **Rust** (a request to a Rust endpoint returns data, not 401).
2. Dashboard renders — `StatsBar`, `ActivePlanCard`, `UnifiedFeed` all populate from Rust.
3. Navigate the shell; create and edit a chunk; confirm it persists.
4. **Visit a page backed by an unported domain** (features) and assert the shell survives — nav present, no blank page. The analysis says every unported call degrades via `?? default` with no `useSuspenseQuery` anywhere; this is where that claim gets tested rather than trusted.

- [ ] **Step 2: Run it**

Run: `cd apps/web && pnpm exec playwright test e2e/critical-path.spec.ts`
Expected: PASS.

- [ ] **Step 3: Prove step 4 is load-bearing**

Temporarily point the features query at a Rust route that 500s rather than 404s. Confirm the shell **still** renders and the assertion still passes — or, if it does not, that is a real finding about degradation and belongs in the report.

- [ ] **Step 4: Stop the servers and confirm port 3000 is clear**

- [ ] **Step 5: Commit**

```bash
git add apps/web/e2e/critical-path.spec.ts
git commit -m "test(web): end-to-end critical path against the Rust backend" -- apps/web/e2e/critical-path.spec.ts apps/web/playwright.config.ts apps/web/package.json pnpm-lock.yaml
```

---

## Exit Criteria

- [ ] `cd apps/web && pnpm exec tsgo -p tsconfig.json --noEmit` → **0 errors**, with only the 9 documented dynamic-index sites still `any`.
- [ ] `cargo test --workspace` green; report the count (baseline **586 passed, 13 ignored**).
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` zero; `cargo fmt --check` clean.
- [ ] `SQLX_OFFLINE=true cargo check --workspace --all-targets` clean.
- [ ] The cookie verifier rejects a forged signature, proven by removing the check and watching the test fail.
- [ ] The dev-user bootstrap tested against an **empty** database.
- [ ] Cookie interop verified at **both** origins, with the Caddy result recorded either way.
- [ ] Playwright critical path passes, including the degraded-domain case.
- [ ] `crates/fubbik-db/migrations/` byte-identical to base; `.sqlx` additive-only.
- [ ] `assets.rs` deleted and its fallback registration removed.
