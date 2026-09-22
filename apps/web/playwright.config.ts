import { defineConfig } from "@playwright/test";

// Shared only between the two servers spawned below (Node and Rust), for this
// test run — never read from or written to any `.env` file. Overriding it
// here (rather than relying on whatever `apps/server/.env` has) is what lets
// Step 3 of the e2e brief flip Rust's copy to a bad value and prove the
// cross-server cookie-verification assertion is load-bearing, without ever
// touching real secrets. Bun's env loading does not override already-set
// process env vars, so this wins over `apps/server/.env`'s own value.
const E2E_BETTER_AUTH_SECRET = "e2e-shared-test-secret-must-be-32-chars-or-more-000";

// Rust's scratch database — NEVER the live one. See `crates/fubbik-db/src/lib.rs`:
// `fubbik_db::connect()` runs Rust's migrations on whatever `DATABASE_URL` it
// is given, so pointing this at the live DB would silently migrate it.
const RUST_DATABASE_URL = process.env.E2E_DATABASE_URL ?? "postgres://postgres:password@localhost:5434/fubbik_rs";

export default defineConfig({
    testDir: "./e2e",
    testIgnore: ["**/components/**", "**/type-tests/**"],
    fullyParallel: false,
    retries: process.env.CI ? 2 : 0,
    use: {
        baseURL: "http://localhost:3001",
        trace: "on-first-retry"
    },
    projects: [
        {
            name: "chromium",
            use: { browserName: "chromium" }
        }
    ],
    // Never attach to developers' existing servers: they may use a live DB,
    // which would defeat the scratch-database guarantee below.
    webServer: [
        {
            // Start Rust first so its migrations have committed before the
            // Node process touches the shared scratch database.
            command: "cargo run -p fubbik -- serve",
            cwd: "../..",
            port: 3100,
            reuseExistingServer: false,
            timeout: 60_000,
            env: {
                DATABASE_URL: RUST_DATABASE_URL,
                BETTER_AUTH_SECRET: E2E_BETTER_AUTH_SECRET,
                NODE_ENV: "production",
                HOST: "127.0.0.1",
                PORT: "3100",
                CORS_ORIGIN: "http://localhost:3001"
            }
        },
        {
            command: "bun run --hot src/index.ts",
            cwd: "../server",
            port: 3000,
            reuseExistingServer: false,
            timeout: 30_000,
            env: {
                BETTER_AUTH_SECRET: E2E_BETTER_AUTH_SECRET,
                BETTER_AUTH_URL: "http://localhost:3000",
                CORS_ORIGIN: "http://localhost:3001",
                NODE_ENV: "test",
                PORT: "3000",
                // Node and Rust must see the SAME `user`/`session` rows for
                // the cross-server cookie check to mean anything — Rust's
                // `CurrentUser` extractor looks the session token up in its
                // own `state.pool`, which is pinned to the scratch DB below.
                // `crates/fubbik-db/migrations/0001_init.sql` is a full copy
                // of Node's schema (including `user`/`session`), so Node can
                // run against it unmodified. Still the scratch DB, never the
                // live one — see RUST_DATABASE_URL's own comment.
                DATABASE_URL: RUST_DATABASE_URL
            }
        },
        {
            command: "bun run vite dev",
            port: 3001,
            reuseExistingServer: false,
            timeout: 30_000,
            env: {
                VITE_FUBBIK_DISABLE_IMPLICIT_DEV_UX: "true",
                VITE_API_ORIGIN: "http://localhost:3100",
                API_PROXY_TARGET: "http://127.0.0.1:3100"
            }
        }
    ]
});
