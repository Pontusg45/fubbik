import { expect, type Page, test } from "@playwright/test";

// This suite proves the Rust slice end-to-end along the path a real user
// takes: sign in through Node (which still owns Better Auth), then exercise
// dashboard, chunk, and feature domains through the Rust API. Node and Rust
// share the scratch database so Rust must also honour Node's session cookie.

const TEST_USER = {
    name: "Critical Path User",
    email: `critical-path-${Date.now()}@example.com`,
    password: "testpassword123"
};

// Rust's own port — see playwright.config.ts's first `webServer` entry.
// `GET /api/auth/get-session` only exists on Rust (Node's equivalent is
// `GET /api/me` on :3000), so a 200 here can only mean Rust independently
// verified the better-auth cookie Node set and looked up the same user.
const RUST_ORIGIN = "http://localhost:3100";

async function waitForHydration(page: Page) {
    await page.waitForLoadState("networkidle");
}

async function signIn(page: Page) {
    await page.goto("/login");
    await waitForHydration(page);
    await page.getByRole("button", { name: "Already have an account? Sign In" }).click();
    await page.getByLabel("Email").fill(TEST_USER.email);
    await page.getByLabel("Password").fill(TEST_USER.password);
    await page.locator("form").getByRole("button", { name: "Sign In" }).click();
    await page.waitForURL("**/dashboard", { timeout: 15000 });
}

test.describe.serial("Critical path (Rust backend)", () => {
    test("sign up through Node, and Rust honours the session it issued", async ({ page }) => {
        await page.goto("/login");
        await waitForHydration(page);

        await page.getByLabel("Name").fill(TEST_USER.name);
        await page.getByLabel("Email").fill(TEST_USER.email);
        await page.getByLabel("Password").fill(TEST_USER.password);
        await page.getByRole("button", { name: "Sign Up" }).click();
        await page.waitForURL("**/dashboard", { timeout: 15000 });

        // The whole point of this slice: Node issued the session cookie
        // above, and now we hand it to Rust directly. If Rust can't verify
        // the cookie's HMAC (wrong secret, cookie format mismatch, etc.)
        // this 401s instead of returning the real user.
        const response = await page.evaluate(async origin => {
            const res = await fetch(`${origin}/api/auth/get-session`, { credentials: "include" });
            return { status: res.status, body: await res.json().catch(() => null) };
        }, RUST_ORIGIN);

        expect(response.status).toBe(200);
        expect(response.body.email).toBe(TEST_USER.email);
        expect(response.body.name).toBe(TEST_USER.name);
    });

    test("dashboard renders live Rust-backed data", async ({ page }) => {
        const rustStats = page.waitForResponse(response => response.url().startsWith(RUST_ORIGIN) && response.url().includes("/api/stats"));
        await signIn(page);
        await waitForHydration(page);
        expect((await rustStats).status()).toBe(200);

        // StatsBar: past the loading skeleton, showing a real (Rust /api/stats)
        // count rather than blank/stuck-loading.
        await expect(page.getByText(/\d+ chunks/)).toBeVisible();
        await expect(page.getByText(/\d+ connections/)).toBeVisible();

        // ActivePlanCard: a brand-new user has no plans, so the real,
        // successful response from Rust's /api/plans is the explicit empty
        // state (a genuine "Start one" link) — not a stuck skeleton, and
        // not the ErrorBoundary fallback.
        await expect(page.getByText("No active plan —")).toBeVisible();
        await expect(page.getByRole("link", { name: /Start one/ })).toBeVisible();
        await expect(page.getByText("Failed to load active plan")).not.toBeVisible();

        // UnifiedFeed: past "Loading…", not the ErrorBoundary fallback. A
        // brand-new user also has nothing in their feed yet, so the real,
        // successful state is the explicit empty prompt.
        await expect(page.getByText("Loading…")).not.toBeVisible({ timeout: 20_000 });
        await expect(page.getByText("Failed to load feed")).not.toBeVisible();
        await expect(page.getByText("Nothing happening yet.")).toBeVisible();
    });

    test("create and edit a chunk, and it persists across a reload", async ({ page }) => {
        await signIn(page);

        const chunkTitle = `Critical path chunk ${Date.now()}`;
        const chunkContent = "Original content from the critical-path e2e test.";
        const tagName = `critpath-${Date.now()}`;
        const alternativeA = "Option A considered";
        const alternativeB = "Option B considered";
        const consequencesText = "Easier onboarding, harder rollback.";

        // Exercise the extended Rust DTO, including fields that used to be
        // silently dropped during the migration.
        await page.goto("/chunks/new");
        await waitForHydration(page);
        await page.locator("#chunk-title").fill(chunkTitle);
        await page.getByPlaceholder("Write your content...").fill(chunkContent);

        await page.locator("#chunk-tags").fill(tagName);
        await page.locator("#chunk-tags").press("Enter");
        await expect(page.getByText(tagName, { exact: true })).toBeVisible();

        await page.getByRole("button", { name: "Decision Context" }).click();
        await page.locator("#chunk-alternatives").fill(`${alternativeA}, ${alternativeB}`);
        await page.locator("#chunk-consequences").fill(consequencesText);

        const createChunk = page.waitForResponse(
            response =>
                response.url().startsWith(RUST_ORIGIN) && response.url().endsWith("/api/chunks") && response.request().method() === "POST"
        );
        await page.getByRole("button", { name: /Create Chunk/ }).click();
        expect((await createChunk).status()).toBe(201);
        await page.waitForURL(/\/chunks\/[^/]+$/, { timeout: 15000 });

        await expect(page.getByRole("heading", { level: 1, name: chunkTitle })).toBeVisible();
        await expect(page.getByText(chunkContent)).toBeVisible();
        await expect(page.getByText(tagName, { exact: true })).toBeVisible();

        // Alternatives/consequences live behind the "More context" drawer's
        // "Context" tab (chunks.$chunkId.tsx / more-context-context-tab.tsx).
        await page.getByRole("button", { name: /More context/ }).click();
        await page.getByRole("button", { name: "Context" }).click();
        await expect(page.getByText(alternativeA)).toBeVisible();
        await expect(page.getByText(alternativeB)).toBeVisible();
        await expect(page.getByText(consequencesText)).toBeVisible();
        await page.keyboard.press("Escape");

        // Edit through Rust and verify the response before checking the UI.
        const updatedTitle = `${chunkTitle} (edited)`;
        const updatedContent = "Updated content, saved through the edit page.";
        await page.getByRole("link", { name: "Edit" }).click();
        await waitForHydration(page);
        await page.locator("#edit-title").fill(updatedTitle);
        await page.getByPlaceholder("Write your content...").fill(updatedContent);
        const updateChunk = page.waitForResponse(
            response => response.url().startsWith(RUST_ORIGIN) && response.request().method() === "PATCH"
        );
        await page.getByRole("button", { name: "Save Changes" }).click();
        expect((await updateChunk).status()).toBe(200);
        await page.waitForURL(/\/chunks\/[^/]+$/, { timeout: 15000 });

        await expect(page.getByRole("heading", { level: 1, name: updatedTitle })).toBeVisible();
        await expect(page.getByText(updatedContent)).toBeVisible();

        // Persistence: reload wipes any client-only state; content, tags,
        // and decision-context fields must come back from the server, not
        // just React Query's cache.
        await page.reload();
        await waitForHydration(page);
        await expect(page.getByRole("heading", { level: 1, name: updatedTitle })).toBeVisible();
        await expect(page.getByText(updatedContent)).toBeVisible();
        await expect(page.getByText(tagName, { exact: true })).toBeVisible();

        await page.getByRole("button", { name: /More context/ }).click();
        await page.getByRole("button", { name: "Context" }).click();
        await expect(page.getByText(alternativeA)).toBeVisible();
        await expect(page.getByText(alternativeB)).toBeVisible();
        await expect(page.getByText(consequencesText)).toBeVisible();
    });

    test("features page shows content round-tripped through Rust", async ({ page }) => {
        await signIn(page);

        const featureName = `critical-path-feature-${Date.now()}`;

        await page.goto("/features");
        await waitForHydration(page);

        await page.getByRole("button", { name: "New Feature" }).click();
        await page.getByLabel("Name").fill(featureName);
        const createFeature = page.waitForResponse(
            response =>
                response.url().startsWith(RUST_ORIGIN) && response.url().endsWith("/api/features") && response.request().method() === "POST"
        );
        await page.getByRole("button", { name: "Create" }).click();
        expect((await createFeature).status()).toBe(201);

        // Real content, not merely "the page didn't crash": the feature we
        // just created, round-tripped through Rust and back,
        // showing its actual name and a real delta count.
        await expect(page.getByText(featureName)).toBeVisible();
        await expect(page.getByText("0 deltas")).toBeVisible();
    });
});
