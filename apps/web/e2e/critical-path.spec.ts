import { expect, type Page, test } from "@playwright/test";

// This suite proves the Rust slice end-to-end along the path a real user
// takes: sign in through Node (which still owns auth/SSR), then exercise
// domains that are ported to Rust (dashboard stats/plans/activity) alongside
// a domain that is NOT yet ported (features, served via `legacyApi` ->
// Node). Chunk create/edit both route through legacyApi/Node too — Rust's
// CreateChunkBody/UpdateChunkBody don't accept tags/alternatives/
// consequences yet — see `apps/web/src/utils/api.ts` for the hybrid
// client's routing rules.

const TEST_USER = {
    name: "Critical Path User",
    email: `critical-path-${Date.now()}@example.com`,
    password: "testpassword123"
};

// Rust's own port — see playwright.config.ts's third `webServer` entry.
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
        await signIn(page);
        await waitForHydration(page);

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
        // brand-new user also has nothing in their feed yet, so — same
        // reasoning as ActivePlanCard — the real successful state is the
        // explicit empty prompt.
        //
        // NB: `UnifiedFeed`'s proposals query (unified-feed.tsx) currently
        // calls `api.api.proposals` — the Rust client — instead of
        // `legacyApi.api.proposals`; "proposals" is one of the domains
        // documented as unported in `@/utils/api`, so that call 404s and
        // React Query burns through its retry/backoff before the query
        // settles. That's a pre-existing bug (out of scope for this task),
        // not flakiness in this assertion — the generous timeout below is
        // to tolerate it, not paper over a real hang.
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

        // Create. POST now goes through legacyApi/Node, not Rust — see the
        // note in chunks.new.tsx. Rust's CreateChunkBody has no
        // tags/alternatives/consequences fields, so a request built against
        // Rust would 200 while silently dropping all three; this test
        // exercises exactly the fields Rust's DTO is missing, not just
        // title/content (which Rust does accept and would have let this
        // pass even before the C1 fix).
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

        await page.getByRole("button", { name: /Create Chunk/ }).click();
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

        // Edit (PATCH goes through legacyApi/Node — see the note in
        // chunks.$chunkId_.edit.tsx).
        const updatedTitle = `${chunkTitle} (edited)`;
        const updatedContent = "Updated content, saved through the edit page.";
        await page.getByRole("link", { name: "Edit" }).click();
        await waitForHydration(page);
        await page.locator("#edit-title").fill(updatedTitle);
        await page.getByPlaceholder("Write your content...").fill(updatedContent);
        await page.getByRole("button", { name: "Save Changes" }).click();
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

    test("features page (unported domain, routed to Node via legacyApi) shows real content", async ({ page }) => {
        await signIn(page);

        const featureName = `critical-path-feature-${Date.now()}`;

        await page.goto("/features");
        await waitForHydration(page);

        await page.getByRole("button", { name: "New Feature" }).click();
        await page.getByLabel("Name").fill(featureName);
        await page.getByRole("button", { name: "Create" }).click();

        // Real content, not merely "the page didn't crash": the feature we
        // just created, round-tripped through Node's own domain and back,
        // showing its actual name and a real delta count.
        await expect(page.getByText(featureName)).toBeVisible();
        await expect(page.getByText("0 deltas")).toBeVisible();
    });
});
