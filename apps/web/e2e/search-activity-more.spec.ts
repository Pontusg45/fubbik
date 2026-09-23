import type { Page } from "@playwright/test";

import { siteTest as test } from "./support/site-test";
import { expect } from "./support/test";

const queryInput = (page: Page) => page.getByPlaceholder("type:note tag:architecture near:chunk-id:2 NOT text:deprecated");
const clause = (field: string, value: string) => ({ field, operator: "eq", value });

async function seedSaved(page: Page, origin: string, name: string) {
    const response = await page.request.post(`${origin}/api/search/saved`, {
        data: { name, query: { clauses: [clause("type", "note")], join: "and" } }
    });
    expect(response.status()).toBe(200);
    return (await response.json()) as { id: string };
}

async function openSearch(page: Page) {
    await page.goto("/search");
    await page.waitForLoadState("networkidle");
}

test("a typed query creates a filter pill and URL query", async ({ page }) => {
    // Given an empty search page.
    await openSearch(page);
    // When the user submits a type query.
    await queryInput(page).fill("type:note");
    await queryInput(page).press("Enter");
    // Then a type filter appears and the URL stores the query.
    await expect(page.getByText("note", { exact: true }).first()).toBeVisible();
    await expect(page).toHaveURL(/q=type%3Anote|q=type%3Anote/);
});

test("a search URL restores its clauses after reload", async ({ page }) => {
    // Given a URL with a type clause.
    await page.goto("/search?q=type%3Anote");
    // When the page reloads.
    await page.reload();
    // Then the type clause is reconstructed.
    await expect(page.getByText("note", { exact: true }).first()).toBeVisible();
    await expect(page.getByRole("button", { name: "Remove filter" })).toHaveCount(1);
});

test("an example query loads a filter clause", async ({ page }) => {
    // Given the empty search state.
    await openSearch(page);
    // When the reference-doc example is selected.
    await page.getByRole("button", { name: /All reference docs/ }).click();
    // Then a reference type clause appears.
    await expect(page.getByText("reference", { exact: true }).first()).toBeVisible();
    await expect(page.getByRole("button", { name: "Remove filter" })).toHaveCount(1);
});

test("Clear removes all search clauses", async ({ page }) => {
    // Given a type clause loaded from an example.
    await openSearch(page);
    await page.getByRole("button", { name: /All reference docs/ }).click();
    // When Clear is clicked.
    await page.getByRole("button", { name: "Clear" }).click();
    // Then the initial empty state returns.
    await expect(page.getByRole("button", { name: "Remove filter" })).toHaveCount(0);
    await expect(page.getByText("Try an example query", { exact: true })).toBeVisible();
});

test("Add filter can create a type clause", async ({ page }) => {
    // Given an empty search page.
    await openSearch(page);
    // When a type filter is chosen from the filter menu.
    await page.getByRole("button", { name: "Add filter" }).click();
    await page.getByRole("button", { name: "Type ›", exact: true }).click();
    await page.getByRole("button", { name: "document", exact: true }).click();
    // Then a document clause appears.
    await expect(page.getByRole("button", { name: "Remove filter" })).toHaveCount(1);
    await expect(page.getByText("document", { exact: true }).first()).toBeVisible();
});

test("a single search filter can be removed", async ({ page }) => {
    // Given a URL-backed type filter.
    await page.goto("/search?q=type%3Anote");
    await expect(page.getByRole("button", { name: "Remove filter" })).toHaveCount(1);
    // When its Remove button is clicked.
    await page.getByRole("button", { name: "Remove filter" }).click();
    // Then the filter pill disappears.
    await expect(page.getByRole("button", { name: "Remove filter" })).toHaveCount(0);
});

test("two search clauses can toggle from AND to OR", async ({ page }) => {
    // Given a search URL containing two clauses.
    await page.goto("/search?q=type%3Anote%20origin%3Ahuman");
    await expect(page.getByRole("button", { name: "Remove filter" })).toHaveCount(2);
    // When the conjunction is toggled.
    await page.getByRole("button", { name: "and", exact: true }).click();
    // Then the OR label is displayed.
    await expect(page.getByRole("button", { name: "or", exact: true })).toBeVisible();
});

test("saving a query adds it to the Saved queries menu", async ({ page, network }) => {
    // Given a type filter in the query builder.
    await openSearch(page);
    await page.getByRole("button", { name: /All reference docs/ }).click();
    const name = `Saved search ${crypto.randomUUID().slice(0, 6)}`;
    // When Save is clicked and a name is entered.
    page.once("dialog", dialog => dialog.accept(name));
    await network.perform({ method: "POST", path: "/api/search/saved", status: 200 }, () =>
        page.getByRole("button", { name: "Save", exact: true }).click()
    );
    // Then the named query appears in the menu.
    await page.getByRole("button", { name: "Saved queries" }).click();
    await expect(page.getByRole("menuitem", { name: new RegExp(name) })).toBeVisible();
});

test("a saved query can be loaded and deleted", async ({ page, network }) => {
    // Given a saved type query.
    const name = `Load query ${crypto.randomUUID().slice(0, 6)}`;
    const saved = await seedSaved(page, network.origin, name);
    await openSearch(page);
    // When the saved query is loaded.
    await page.getByRole("button", { name: "Saved queries" }).click();
    await page.getByRole("menuitem", { name: new RegExp(name) }).click();
    // Then its clause appears, and deleting it removes the menu item.
    await expect(page.getByRole("button", { name: "Remove filter" })).toHaveCount(1);
    await page.getByRole("button", { name: "Saved queries" }).click();
    await network.perform({ method: "DELETE", path: `/api/search/saved/${saved.id}`, status: 200 }, () =>
        page
            .getByRole("menuitem", { name: new RegExp(name) })
            .getByRole("button", { name: "Delete saved query" })
            .click()
    );
    await expect(page.getByRole("button", { name: "Saved queries" })).toHaveCount(0);
});

test("activity entity filters exclude events of other types", async ({ page, network }) => {
    // Given a plan creation event in the activity feed.
    const planResponse = await page.request.post(`${network.origin}/api/plans`, { data: { title: "Activity plan" } });
    expect(planResponse.status()).toBe(200);
    await page.goto("/activity");
    await page.waitForLoadState("networkidle");
    await expect(page.getByText("Activity plan", { exact: true })).toBeVisible();
    // When the Chunks entity filter is selected.
    await page.getByRole("button", { name: "Chunks", exact: true }).click();
    // Then the non-chunk event is hidden.
    await expect(page.getByText("Activity plan", { exact: true })).toHaveCount(0);
    await expect(page.getByText("No activity yet.", { exact: true })).toBeVisible();
});
