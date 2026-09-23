import { defineConfig, type ReporterDescription } from "@playwright/test";

// Dedicated authentication secret for the isolated test server.
const E2E_BETTER_AUTH_SECRET = "e2e-shared-test-secret-must-be-32-chars-or-more-000";

// Rust's scratch database — NEVER the live one. See `crates/fubbik-db/src/lib.rs`:
// `fubbik_db::connect()` runs Rust's migrations on whatever `DATABASE_URL` it
// is given, so pointing this at the live DB would silently migrate it.
const RUST_DATABASE_URL = process.env.E2E_DATABASE_URL ?? "postgres://postgres:password@localhost:5434/fubbik_rs";
const reporters: ReporterDescription[] = [["list"], ["html", { outputFolder: "playwright-report/e2e", open: "never" }]];
if (process.env.E2E_STEP_REPORT)
    reporters.push(["./scripts/step-file-reporter.mjs", { format: process.env.E2E_STEP_REPORT, outputDir: "test-results/e2e" }]);

export default defineConfig({
    testDir: "./e2e",
    testIgnore: ["**/components/**", "**/type-tests/**"],
    fullyParallel: false,
    retries: process.env.CI ? 2 : 0,
    outputDir: "test-results/e2e",
    reporter: reporters,
    use: {
        baseURL: "http://localhost:3001",
        trace: "retain-on-failure",
        screenshot: "only-on-failure"
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
            // Rust owns authentication and application routes, and migrates the scratch DB.
            command: "cargo run -p fubbik -- serve",
            cwd: "../..",
            port: 3100,
            reuseExistingServer: false,
            timeout: 300_000,
            env: {
                // Compile from checked-in SQL metadata before migrating the empty scratch DB.
                SQLX_OFFLINE: "true",
                DATABASE_URL: RUST_DATABASE_URL,
                BETTER_AUTH_SECRET: E2E_BETTER_AUTH_SECRET,
                NODE_ENV: "production",
                HOST: "127.0.0.1",
                PORT: "3100",
                CORS_ORIGIN: "http://localhost:3001"
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
