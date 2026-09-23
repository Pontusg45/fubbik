import { defineConfig, devices, type ReporterDescription } from "@playwright/test";

const reporters: ReporterDescription[] = [["list"], ["html", { outputFolder: "playwright-report/components", open: "never" }]];
if (process.env.E2E_STEP_REPORT)
    reporters.push(["./scripts/step-file-reporter.mjs", { format: process.env.E2E_STEP_REPORT, outputDir: "test-results/components" }]);

export default defineConfig({
    testDir: "./e2e/components",
    testMatch: "**/*.spec.ts",
    fullyParallel: true,
    forbidOnly: !!process.env.CI,
    retries: process.env.CI ? 2 : 0,
    // Keep three-browser runs within the same resource budget locally and in CI.
    workers: 2,
    reporter: reporters,
    outputDir: "test-results/components",
    use: { baseURL: "http://127.0.0.1:4178", trace: "retain-on-failure", screenshot: "only-on-failure" },
    projects: [
        { name: "chromium", use: { ...devices["Desktop Chrome"] } },
        { name: "firefox", use: { ...devices["Desktop Firefox"] } },
        { name: "webkit", use: { ...devices["Desktop Safari"] } }
    ],
    webServer: {
        command: "pnpm exec vite --config e2e/components/fixture/vite.config.ts",
        url: "http://127.0.0.1:4178",
        reuseExistingServer: false,
        timeout: 60_000
    }
});
