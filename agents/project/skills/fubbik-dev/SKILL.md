---
name: fubbik-dev
description:
    Use when writing code in the fubbik repo — the Rust crates (axum routes, sqlx repositories, migrations, utoipa/OpenAPI, cargo
    tests) or the TanStack Router web app and its generated API client — to find where code goes, copy the real patterns, and avoid
    the pitfalls that silently break builds, feature switches or generated files.
---

# Fubbik Development Patterns

Where code goes, the patterns to copy, and the pitfalls. CLAUDE.md is the full reference (concepts, endpoints, CLI). The retired
TypeScript backend (Effect, Elysia, Drizzle, Eden) has been removed; comments citing `legacy/…` paths point into git history. Never copy patterns from it.

## Quick reference

```bash
just dev                 # Rust API on 3000 + web on 3001 (same as `pnpm dev`)
just rust-test-fast      # cargo fmt --check + the database-free crates
just rust-test           # full Rust suite against the PG18 + pgvector + AGE test adapter
just test-changed        # only crates changed since main, plus reverse deps (--dry-run to preview; needs jq)
pnpm run check-types     # tsgo across the TS packages
pnpm lint                # oxlint (web: also scripts/check-legacy-api.mjs)
cargo run -q -p review-harness -- run   # repo convention checks (review-rules.toml)
```

## Crate layering

Lowest first. A crate may only depend on crates above it in this list.

```
fubbik-core        errors (AppError/AppResult), health, scoring, token budgeting, glob, formatting — no I/O
fubbik-contracts   wire types shared by the API and its Rust clients (system.rs: Stats/HealthResponse, matrix.rs)
fubbik-config      fubbik.toml loading, the Feature enum
fubbik-db          sqlx repositories (src/repo/*.rs), migrations, seed
fubbik-ai          Ollama transport (OllamaClient)
fubbik-api         axum: one module per domain (src/<domain>/{routes,service,dto}.rs), AppState, router()
fubbik             the binary: serve, seed, mcp, openapi + CLI dispatch (crates/fubbik/src/main.rs)
```

`fubbik-cli` and `fubbik-mcp` are HTTP clients of the API. They never depend on `fubbik-db` or `fubbik-api`. The `fubbik` binary
links all of them.

## Adding a backend domain

Use `favorites` as the model. It is small and has every layer: `crates/fubbik-db/src/repo/favorite.rs`,
`crates/fubbik-api/src/favorites/{mod,dto,routes,service}.rs`, `crates/fubbik-api/tests/favorites.rs`.

```
1. Migration   crates/fubbik-db/migrations/NNNN_<name>.sql     next number after the last file; forward-only
2. Repository  crates/fubbik-db/src/repo/<name>.rs             + `pub mod <name>;` in crates/fubbik-db/src/repo/mod.rs
3. Service     crates/fubbik-api/src/<domain>/service.rs       business rules, AppError variants
4. DTOs        crates/fubbik-api/src/<domain>/dto.rs           request/response types (Deserialize/Serialize + ToSchema)
5. Routes      crates/fubbik-api/src/<domain>/routes.rs        #[utoipa::path] handlers + `pub fn router()`
6. Module      `pub mod <domain>;` and `.merge(<domain>::routes::router())` in crates/fubbik-api/src/lib.rs
7. OpenAPI     add each handler to `paths(...)` and each type to `components(schemas(...))` in crates/fubbik-api/src/openapi.rs
8. Regenerate  openapi.json, packages/client/src/api-types.ts, .sqlx/ (see "Generated files")
9. Tests       crates/fubbik-db/tests/<name>.rs, crates/fubbik-api/tests/<domain>.rs
```

A wire type that a Rust client (the CLI) also needs goes in `fubbik-contracts` instead of `dto.rs`.

### Schema: sqlx migrations are the authority

The schema is the migrations in `crates/fubbik-db/migrations/`, embedded by `sqlx::migrate!("./migrations")`
(`crates/fubbik-db/src/migration.rs`) and applied when the server or seed connects. There is no `db:push`. The Drizzle schema in
`packages/db` is a read-only reference for `pnpm db:studio`: do not edit it to change the schema. IDs are `text` primary keys
(`fubbik_db::new_id()`). Migrations never depend on `fubbik.toml`: the schema is always complete.

### Repository (`fubbik-db`)

Plain async functions taking `&PgPool`, returning `AppResult<T>`. Compile-checked queries with `sqlx::query_as!`. Row types derive
`sqlx::FromRow`, `serde::Serialize` with `camelCase`, and `utoipa::ToSchema`. From `repo/favorite.rs`:

```rust
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Favorite {
    pub id: String,
    pub user_id: String,
    pub chunk_id: String,
    pub order: i32,
    #[schema(value_type = chrono::NaiveDateTime)]
    pub created_at: UtcTimestamp,
}

pub async fn list(pool: &PgPool, user_id: &str) -> AppResult<Vec<Favorite>> {
    let rows = sqlx::query_as!(
        Favorite,
        r#"SELECT id, user_id, chunk_id, "order", created_at AS "created_at: UtcTimestamp"
           FROM user_favorite
           WHERE user_id = $1
           ORDER BY "order" ASC, id ASC"#,
        user_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}
```

Scope every query by `user_id` (or join through an owned row). `?` converts `sqlx::Error` into `AppError::Database`.

### Service (`fubbik-api/src/<domain>/service.rs`)

Composes repository calls and returns `AppError::{NotFound, Auth, Validation, Conflict, External}`. From `favorites/service.rs`:

```rust
pub async fn add(pool: &PgPool, user_id: &str, chunk_id: &str) -> AppResult<Option<Favorite>> {
    if chunk::find_by_id(pool, user_id, chunk_id).await?.is_none() {
        return Err(AppError::NotFound("Chunk".into()));
    }
    // ...
    let id = fubbik_db::new_id();
    favorite::add(pool, &id, user_id, chunk_id, next_order).await
}
```

`ApiError` (`crates/fubbik-api/src/error.rs`) maps these to statuses: Validation 400, Auth 401, NotFound 404, Conflict 409,
UnsupportedMediaType 415, Database 500 (503 on pool timeout), External 502. Database errors are logged and returned as a generic message.

### Route (`fubbik-api/src/<domain>/routes.rs`)

Handlers take `State<AppState>` and `CurrentUser` (401 when unauthenticated) and return `ApiResult<...>`. Take request bodies and query
strings through `crate::extract::{Json, Query}`, not axum's own, so parse failures come back as `{"message": ...}` JSON (400, or 415
for a missing content type) instead of axum's plain-text rejections. From
`favorites/routes.rs`:

```rust
use crate::extract::Json as ReqJson;

#[utoipa::path(post, path = "/api/favorites", request_body = CreateFavoriteBody,
    responses((status = 201, body = Option<Favorite>), (status = 404)))]
pub async fn add_favorite(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ReqJson(body): ReqJson<CreateFavoriteBody>,
) -> ApiResult<(StatusCode, Json<Option<Favorite>>)> {
    let favorite = service::add(&state.pool, &user.id, &body.chunk_id).await?;
    Ok((StatusCode::CREATED, Json(favorite)))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/favorites", get(list_favorites).post(add_favorite))
        .route("/api/favorites/reorder", put(reorder_favorites))
        .route("/api/favorites/{chunkId}", axum::routing::delete(remove_favorite))
}
```

Path parameters use axum's `{name}` syntax, and the same name in `#[utoipa::path(params(...))]`.

## Generated files: regenerate, never hand-edit

| File                                       | Regenerate with                                     | Guarded by                                                   |
| ------------------------------------------ | --------------------------------------------------- | ------------------------------------------------------------ |
| `openapi.json`                             | `cargo run -q -p fubbik -- openapi > openapi.json`  | `committed_openapi_json_is_current` in `crates/fubbik-api/tests/openapi.rs` |
| `packages/client/src/api-types.ts`         | `pnpm gen:api` (after `openapi.json`)               | `packages/client/src/api-types.staleness.test.ts`            |
| `.sqlx/query-*.json`                       | `cargo sqlx prepare --workspace` (needs a live `DATABASE_URL`) | `SQLX_OFFLINE=true` builds in CI and `just` recipes |
| `apps/web/src/routeTree.gen.ts`            | the TanStack Router Vite plugin (`pnpm dev:web` / `pnpm build`) | its own header                                  |

Order after an API change: Rust compiles → `openapi.json` → `pnpm gen:api` → web code that uses the new types.

## Feature-gated groups

`fubbik.toml` switches whole groups (`plans`, `requirements`, `matrices`, `coordination`, `learning_paths`, `overlays`, `ai`). A route in
an existing group goes in that group's `if switches.enabled(Feature::...)` block in `router()` (`crates/fubbik-api/src/lib.rs`), not
in the always-on block. A new group touches every surface (the full checklist is `docs/extending.md`, "Add a feature group"):

1. `crates/fubbik-config/src/lib.rs`: a `Feature` variant, `Feature::ALL`, `key()`, and `requires()` if it depends on another group.
2. `crates/fubbik-api/src/lib.rs`: wrap its routers in `if switches.enabled(Feature::X)`. In `crates/fubbik/src/main.rs`, gate its
   background jobs the same way (see the `Feature::Matrices` check around `spawn_behavior_sync`).
3. `fubbik.example.toml`: document the key.
4. Web, `apps/web/src/features/nav/use-capabilities.ts`: add it to `Feature`, `FEATURE_LABEL` and `FEATURE_BY_ROUTE`. That one map
   drives the nav, the command palette (`filterNavItems`) and the turned-off page (`FeatureRouteGuard`, mounted as `InnerWrap` in
   `apps/web/src/router.tsx`). Gate a core page's query to the group with `useFeatureEnabled(...)`, skipping only on `false`.
5. MCP, `crates/fubbik-mcp/src/features.rs`: map its tools in `feature_for_tool`.
6. CLI: wrap the command group's `run` in `capabilities::explain_not_found(client.base_url(), "<key>", error)` on a 404, as
   `crates/fubbik-cli/src/commands/plan.rs` does.

## Tests

- Database tests use `#[sqlx::test]`, which creates a fresh database per test and passes a `PgPool`. In `fubbik-db` the default
  migrations apply. In `fubbik-api` they are named: `#[sqlx::test(migrations = "../fubbik-db/migrations")]`.
- The database is the canonical adapter: `./scripts/test/rust-test-db.sh start|stop|url` (`just rust-test-db-start`), PG18 with
  pgvector, AGE and ICU in Docker. `just rust-test` manages it for you. To run one database test directly:
  `./scripts/test/rust-test-db.sh start && DATABASE_URL="$(./scripts/test/rust-test-db.sh url)" cargo test -p fubbik-api --test favorites`.
- HTTP tests in `crates/fubbik-api/tests/` drive the real router through `mod common;` (`tests/common/mod.rs`):
  `TestApp::new(pool)`, `app.signup(email, name)`, `app.get/post/patch/put/delete(&user, path, ...)`, `TestApp::json(response)`.
- Mark every test body with `// Given`, `// When`, `// Then`. The review harness's `given-when-then` check enforces it.
- Ollama is faked with `wiremock` and injected through `AppState.ai`. Never read `OLLAMA_URL` in a test. From `tests/ai.rs`:

```rust
let server = MockServer::start().await;
Mock::given(method("POST"))
    .and(path("/api/generate"))
    .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({ "response": response })))
    .mount(&server)
    .await;
let app = TestApp::with_ai(pool, fubbik_ai::OllamaClient::new(server.uri()));
```

- CLI client tests (`crates/fubbik-cli/tests/client_*.rs`) and MCP tests fake the API itself with `wiremock`. They need no database.
- Web: vitest for units (`*.test.ts` beside the code), Playwright e2e in `apps/web/e2e/` (its own port, 3100).

## Web app (`apps/web`)

A client-rendered TanStack Router SPA built by Vite: `src/main.tsx` renders `getRouter()` from `src/router.tsx`. There is no SSR and no
TanStack Start.

```
Route       apps/web/src/routes/<path>.tsx         createFileRoute("/<path>"); routeTree.gen.ts updates itself
Feature     apps/web/src/features/<name>/          components and hooks (use-<name>.ts)
Shared UI   apps/web/src/components/ui/            PageContainer/PageHeader/PageLoading/PageEmpty in page.tsx
```

Data goes through the generated client: `api` (`src/utils/api.ts`, `createClient` from `@fubbik/client`) with paths that mirror the
URL, unwrapped by `unwrapEden` (`src/utils/eden.ts`), inside TanStack Query. The name is historical: Eden itself is gone. From
`features/chunks/use-favorites.ts`:

```ts
const favoritesQuery = useQuery({
    queryKey: ["favorites"],
    queryFn: async () => unwrapEden(await api.api.favorites.get())
});

const removeMutation = useMutation({
    mutationFn: async (chunkId: string) => unwrapEden(await api.api.favorites({ chunkId }).delete()),
    onSettled: () => queryClient.invalidateQueries({ queryKey: ["favorites"] })
});
```

UI components are shadcn on `@base-ui/react`: use the `render` prop (`<DropdownMenuItem render={<Link to="/features" />}>`), never
`asChild`. `DropdownMenuSeparator` and `DropdownMenuLabel` are plain `div`s. arktype is only allowed in the auth forms and
`packages/env`. The review harness checks both rules.

## CLI and MCP

- CLI command: module in `crates/fubbik-cli/src/commands/<name>.rs`, `pub mod` in `commands/mod.rs`, a variant in `enum Command` in
  `crates/fubbik-cli/src/lib.rs` plus its match arm. HTTP calls go on `Client` in `crates/fubbik-cli/src/client/`. Support `--json` and
  `--quiet` via `output::render` (see `commands/stats.rs`).
- MCP tool: `crates/fubbik-mcp/src/tools/{plan,matrix,coordination}.rs` for grouped tools (`NAMES`, `call`, `definitions()`), the rest in
  `src/lib.rs`. Tools call the HTTP API through `ApiClient`, never the database.

## Pitfalls

### Silent bugs

| Pitfall | Do this instead |
| --- | --- |
| `features: Default::default()` in an `AppState` outside tests | `Default` turns every feature on and ignores `fubbik.toml`. `serve` (`crates/fubbik/src/main.rs`) passes `config.features` from `fubbik_config::Config::load()`. Use `Default` only in tests. |
| A feature-gated route added on one surface only | Gate it in `router()`, list its page in `FEATURE_BY_ROUTE`, and map its tools in `feature_for_tool`. Otherwise the nav, the route guard and MCP disagree with the server. |
| React hook dependency arrays missing a value | Run `pnpm exec oxlint <files>` on every changed TS file. The root `.oxlintrc.json` errors on `exhaustive-deps`. |
| Hand-editing `openapi.json`, `api-types.ts` or `routeTree.gen.ts` | Regenerate them (table above). Drift tests fail on stale copies. |
| Re-implementing scoring, token counts or glob matching | Use `fubbik_core::score::{score_chunk, budget_chunks}`, `fubbik_core::tokens::estimate_tokens` (o200k_base) and `fubbik_core::glob::glob_match`. |
| Applying feature overlays by hand in context code | `enrich_chunks` already applies them (`resolve_feature_overlays`, `crates/fubbik-api/src/context/service.rs`). |
| Assuming the chunk tag filter is AND | `GET /api/chunks?tags=a,b` is any-of only. AND (`tagMode=all`) exists only on `/api/chunks/grouped`. |

### Breaks builds or clients

| Pitfall | Do this instead |
| --- | --- |
| A readonly `as const` nav array passed to a `T[]` parameter | Accept `readonly T[]`, as `filterNavItems(items: readonly T[], ...)` does. |
| A new or changed `query!` without refreshing `.sqlx/` | `SQLX_OFFLINE=true` builds (CI, `just rust-test-fast`) fail. Run `cargo sqlx prepare --workspace` against a migrated database. |
| A handler or DTO missing from `openapi.rs` | The route works but is missing from `openapi.json` and the TS client. Add it to `paths(...)` / `components(schemas(...))`. |
| Using axum's `Json`/`Query` extractors for input | Use `crate::extract::{Json, Query}` so parse errors are JSON, like every other error. |
| Copying a pattern from the old `legacy/` code (git history) | It is retired TypeScript. The live code is in `crates/` and `apps/web`. |

### Environment

| Pitfall | Do this instead |
| --- | --- |
| Reading `.env*` files | User permissions may deny agents `Read(.env*)`. Don't try to work around it: ask the user, or read `.env.example` and the environment docs. |
| `cargo clean`, or deleting `target/` | `just` exports one `CARGO_TARGET_DIR` (the main checkout's `target/`) that every worktree shares. Never clean it. Plain `cargo` shares it only if you export the same path. Check `df -g .` before large builds. |
| Starting dev servers or killing ports to "check" a change | Use the tests and `cargo run -q -p fubbik -- <cmd> --help`. Ports 3000/3001 may belong to the user's running session. |
