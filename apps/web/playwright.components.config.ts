import { defineConfig, devices } from "@playwright/test";

export default defineConfig({
    testDir: "./e2e/components",
    testMatch: "**/*.spec.ts",
    fullyParallel: true,
    forbidOnly: !!process.env.CI,
    retries: process.env.CI ? 2 : 0,
    ...(process.env.CI ? { workers: 2 } : {}),
    reporter: [["list"], ["html", { outputFolder: "playwright-report/components", open: "never" }]],
    outputDir: "test-results/components",
    use: { baseURL: "http://127.0.0.1:4178", trace: "retain-on-failure", screenshot: "only-on-failure" },
    projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],
    webServer: {
        command: "pnpm exec vite --config e2e/components/fixture/vite.config.ts",
        url: "http://127.0.0.1:4178",
        reuseExistingServer: false,
        timeout: 60_000
    }
});
