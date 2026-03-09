import { defineConfig } from "@playwright/test";

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
            timeout: 30_000
        },
        {
            command: "bun run vite dev",
            port: 3001,
            reuseExistingServer: !process.env.CI,
            timeout: 30_000
        }
    ]
});
