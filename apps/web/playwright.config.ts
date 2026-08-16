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
const RUST_DATABASE_URL = "postgres://postgres:password@localhost:5434/fubbik_rs";

export default defineConfig({
    testDir: "./e2e",
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
    webServer: [
        {
            command: "bun run --hot src/index.ts",
            cwd: "../server",
            port: 3000,
            reuseExistingServer: !process.env.CI,
            timeout: 30_000,
            env: {
                BETTER_AUTH_SECRET: E2E_BETTER_AUTH_SECRET
            }
        },
        {
            command: "bun run vite dev",
            port: 3001,
            reuseExistingServer: !process.env.CI,
            timeout: 30_000,
            env: {
                VITE_FUBBIK_DISABLE_IMPLICIT_DEV_UX: "true"
            }
        },
        {
            // Rust backend, scratch DB only — see RUST_DATABASE_URL above.
            // NODE_ENV=production (mirrored from Node's own convention; read
            // verbatim by crates/fubbik/src/main.rs) disables the
            // implicit-dev-session fallback so cookie verification is real,
            // which is the entire point of the critical-path assertion.
            command: "cargo run -p fubbik -- serve",
            cwd: "../..",
            port: 3100,
            reuseExistingServer: !process.env.CI,
            timeout: 60_000,
            env: {
                DATABASE_URL: RUST_DATABASE_URL,
                BETTER_AUTH_SECRET: E2E_BETTER_AUTH_SECRET,
                NODE_ENV: "production",
                HOST: "127.0.0.1",
                PORT: "3100"
            }
        }
    ]
});
