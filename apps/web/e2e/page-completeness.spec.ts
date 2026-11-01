import { given, openSite, seedChunk, then, when } from "./support/site-scenarios";
import { siteTest as test } from "./support/site-test";
import { expect, test as anonymousTest } from "./support/test";

anonymousTest("landing feature links open matching product sections", async ({ page }) => {
    // Given the public landing page.
    await openSite(page, "/");
    // When a visitor opens the health teaser.
    await when(page, "the health teaser opens", () => page.getByRole("link", { name: /Health Monitoring/ }).click());
    // Then the product page lands on its health section.
    await then(page, "the health section is present", async () => {
        await expect(page).toHaveURL(/\/product#health$/);
        await expect(page.locator("#health").getByRole("heading", { name: "Health Monitoring" })).toBeVisible();
    });
});

test("topic clusters render their own page from Browse", async ({ page }) => {
    // Given an authenticated visitor on Browse.
    await given(page, "the Browse page", () => openSite(page, "/browse"));
    // When Topic Clusters is opened.
    await when(page, "topic clusters opens", () => page.getByRole("link", { name: "Explore topic clusters" }).click());
    // Then the child page renders, including its own empty state or results.
    await then(page, "topic clusters renders", async () => {
        await expect(page).toHaveURL(/\/browse\/clusters$/);
        await expect(page.getByRole("heading", { name: "Topic Clusters" })).toBeVisible();
        await expect(page.getByRole("heading", { name: "Browse", exact: true })).toHaveCount(0);
    });
});

test("mobile navigation exposes the audited pages", async ({ page }) => {
    // Given a narrow authenticated viewport.
    await given(page, "a mobile dashboard", async () => {
        await page.setViewportSize({ width: 375, height: 812 });
        await openSite(page, "/dashboard");
    });
    // When the Manage menu opens.
    await when(page, "the mobile Manage menu opens", async () => {
        await page.getByRole("button", { name: "Open navigation menu" }).click();
        await page.getByRole("button", { name: "Manage" }).click();
    });
    // Then every newly exposed destination is available.
    await then(page, "the destinations are discoverable", async () => {
        for (const name of ["Browse", "Topic Clusters", "Saved Graphs", "Learning Paths", "Coverage", "Matrices", "Review"]) {
            await expect(page.getByRole("link", { name, exact: true })).toBeVisible();
        }
    });
});

test("learning paths can be created, reordered, and deleted in the site", async ({ page, site }) => {
    // Given two saved chunks and the learning-path list.
    const firstTitle = `Path first ${crypto.randomUUID().slice(0, 6)}`;
    const secondTitle = `Path second ${crypto.randomUUID().slice(0, 6)}`;
    await given(page, "the first chunk", () => seedChunk(site, firstTitle));
    await given(page, "the second chunk", () => seedChunk(site, secondTitle));
    await openSite(page, "/learn");
    // When a path is created in the browser.
    await when(page, "a learning path is created", async () => {
        await page.getByRole("link", { name: "New path" }).click();
        await page.getByLabel("Title").fill("Browser reading path");
        await page.getByRole("button", { name: `+ ${firstTitle}` }).click();
        await page.getByRole("button", { name: `+ ${secondTitle}` }).click();
        await page.getByRole("button", { name: "Save learning path" }).click();
    });
    // Then the detail route shows both chunks in order.
    await then(page, "the saved path appears", async () => {
        await expect(page).toHaveURL(/\/learn\/[^/]+$/);
        await expect(page.getByRole("heading", { name: "Browser reading path" })).toBeVisible();
        const entries = page.locator("main ol li a");
        await expect(entries.first()).toContainText(firstTitle);
        await expect(entries.last()).toContainText(secondTitle);
    });
    // When its order is changed through the edit page.
    await when(page, "the second chunk moves first", async () => {
        await page.getByRole("link", { name: "Edit path" }).click();
        await page.getByRole("button", { name: `Move ${secondTitle} up` }).click();
        await page.getByRole("button", { name: "Save learning path" }).click();
        await expect(page).toHaveURL(/\/learn\/[^/]+$/);
        await page.reload();
    });
    // Then order persists and the path can be deleted without deleting chunks.
    await then(page, "the edit persists and deletion is available", async () => {
        await expect(page.locator("main ol li a").first()).toContainText(secondTitle);
        await page.getByRole("button", { name: "Delete path" }).click();
        await page.getByRole("button", { name: "Delete path" }).last().click();
        await expect(page).toHaveURL(/\/learn$/);
    });
});

test("saved graphs can be created, renamed, and deleted", async ({ page, site }) => {
    // Given a saved chunk.
    const chunkTitle = `Graph source ${crypto.randomUUID().slice(0, 6)}`;
    await given(page, "a chunk for a graph", () => seedChunk(site, chunkTitle));
    await openSite(page, "/saved-graphs");
    // When a graph is created with that chunk.
    await when(page, "a graph is saved", async () => {
        await page.getByLabel("Name").fill("Browser saved graph");
        await page.getByRole("button", { name: `+ ${chunkTitle}` }).click();
        await page.getByRole("button", { name: "Save graph" }).click();
    });
    // Then the saved graph opens at its detail URL.
    await then(page, "the graph opens", async () => {
        await expect(page).toHaveURL(/\/graph\/[^/]+$/);
        await expect(page.getByRole("heading", { name: "Browser saved graph" })).toBeVisible();
    });
    // When the user renames and deletes it from the list.
    await when(page, "the graph is renamed and deleted", async () => {
        await openSite(page, "/saved-graphs");
        await page.getByRole("button", { name: "Rename" }).click();
        await page.getByRole("textbox", { name: "Graph name" }).fill("Renamed graph");
        await page.getByRole("button", { name: "Save", exact: true }).click();
        await expect(page.getByRole("link", { name: /Renamed graph/ })).toBeVisible();
        await page.getByRole("button", { name: "Delete", exact: true }).click();
        await page.getByRole("button", { name: "Delete graph" }).click();
    });
    // Then the saved graph list is empty after reload.
    await then(page, "the graph stays deleted", async () => {
        await page.reload();
        await expect(page.getByText("No saved graphs yet.")).toBeVisible();
    });
});
