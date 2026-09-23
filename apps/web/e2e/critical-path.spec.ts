import { expect, test, testAccount } from "./support/test";

// Exercise authentication, dashboard, Chunk and Feature workflows through Rust.

const TEST_USER = testAccount({ name: "Critical Path User" });

test.describe.serial("Critical path (Rust backend)", () => {
    test("sign up through Rust and verify the authenticated session", async ({ screens }) => {
        // Given a new account and the Rust-backed application.
        // When the user signs up and requests the browser session.
        await screens.auth.signUp(TEST_USER);

        // Verify the actual authenticated user, not just a successful redirect.
        const response = await screens.auth.session();

        // Then Rust returns the new authenticated user.
        expect(response.status).toBe(200);
        expect(response.body.user.email).toBe(TEST_USER.email);
        expect(response.body.user.name).toBe(TEST_USER.name);
    });

    test("dashboard renders live Rust-backed data", async ({ page, screens, network }) => {
        // Given an existing account with no chunks or plans.
        // When the user signs in and loads the dashboard.
        await network.perform({ method: "GET", path: "/api/stats", status: 200 }, () => screens.auth.signIn(TEST_USER));
        // Then Rust-backed counts and empty states render successfully.

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

    test("create and edit a chunk, and it persists across a reload", async ({ page, screens }) => {
        // Given an authenticated user and a new chunk with extended metadata.
        await screens.auth.signIn(TEST_USER);

        const chunkTitle = `Critical path chunk ${Date.now()}`;
        const chunkContent = "Original content from the critical-path e2e test.";
        const tagName = `critpath-${Date.now()}`;
        const alternativeA = "Option A considered";
        const alternativeB = "Option B considered";
        const consequencesText = "Easier onboarding, harder rollback.";

        // Exercise the extended Rust DTO, including fields that used to be
        // silently dropped during the migration.
        // When the user creates a document with tags and decision context.
        await screens.chunks.openNew();
        await screens.chunks.form.fill({ title: chunkTitle, content: chunkContent });
        await screens.chunks.tags.add(tagName);
        await screens.chunks.type.choose("document");
        await screens.chunks.setDecisionContext({ alternatives: [alternativeA, alternativeB], consequences: consequencesText });

        await screens.chunks.createAndOpen();
        // Then Rust creates the chunk and the detail page displays it.
        await screens.chunks.expectDetails({ title: chunkTitle, content: chunkContent });
        await expect(page.getByText(tagName, { exact: true })).toBeVisible();

        // Alternatives/consequences live behind the "More context" drawer's
        // "Context" tab (chunks.$chunkId.tsx / more-context-context-tab.tsx).
        await screens.chunks.expectDecisionContext({ alternatives: [alternativeA, alternativeB], consequences: consequencesText });
        await screens.chunks.contextDrawer.dismiss();

        // Edit through Rust and verify the response before checking the UI.
        // When the persisted chunk is reopened and edited.
        const updatedTitle = `${chunkTitle} (edited)`;
        const updatedContent = "Updated content, saved through the edit page.";
        await screens.chunks.openEdit();
        // Then the editor loads its existing tag and type.
        await screens.chunks.tags.expectTag(tagName);
        await screens.chunks.type.expectValue("document");
        // When tags, type, title and content are changed and saved.
        await screens.chunks.tags.add(" Temporary ");
        await screens.chunks.tags.remove("temporary");
        await screens.chunks.tags.add(`${tagName}-edited`);
        await screens.chunks.type.choose("reference");
        await screens.chunks.form.fill({ title: updatedTitle, content: updatedContent });
        await screens.chunks.saveAndOpen();
        // Then Rust accepts the changes and the detail page displays them.
        const updated = { title: updatedTitle, content: updatedContent };
        await screens.chunks.expectDetails(updated);

        // Persistence: reload wipes any client-only state; content, tags,
        // and decision-context fields must come back from the server, not
        // just React Query's cache.
        // When the page is reloaded.
        await screens.chunks.reloadAndExpect(updated);
        await expect(page.getByText(tagName, { exact: true })).toBeVisible();

        // Then added tags persist and removed tags stay absent.
        await expect(page.getByText(`${tagName}-edited`, { exact: true })).toBeVisible();
        await expect(page.getByText("temporary", { exact: true })).toHaveCount(0);
        await screens.chunks.openEdit();
        await screens.chunks.type.expectValue("reference");
        await page.goBack();
        await screens.chunks.expectDecisionContext({ alternatives: [alternativeA, alternativeB], consequences: consequencesText });
    });

    test("features page shows content round-tripped through Rust", async ({ page, screens }) => {
        // Given an authenticated user on the features page.
        await screens.auth.signIn(TEST_USER);

        const featureName = `critical-path-feature-${Date.now()}`;

        await page.goto("/features");
        await page.waitForLoadState("networkidle");

        // When the user creates a named feature.
        await screens.feature.createFeature(featureName);
        // Then Rust persists it and the page shows zero deltas.

        // Real content, not merely "the page didn't crash": the feature we
        // just created, round-tripped through Rust and back,
        // showing its actual name and a real delta count.
        await expect(page.getByText(featureName)).toBeVisible();
        await expect(page.getByText("0 deltas")).toBeVisible();
    });
});
