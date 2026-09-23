import type { Page } from "@playwright/test";

import { siteTest as test } from "./support/site-test";
import { expect } from "./support/test";

async function seed(page: Page, origin: string, title: string, options: { type?: string; tags?: string[] } = {}) {
    const response = await page.request.post(`${origin}/api/chunks`, { data: { title, content: `${title} content`, ...options } });
    expect(response.status()).toBe(201);
    return (await response.json()) as { id: string };
}

test("a search with no matches hides unrelated chunks", async ({ page, network }) => {
    // Given a chunk in the active list.
    await seed(page, network.origin, "Searchable note");
    await page.goto("/chunks");
    await page.waitForLoadState("networkidle");
    await expect(page.getByText("Searchable note", { exact: true })).toBeVisible();
    // When the user searches for an absent phrase.
    await page.getByPlaceholder("Search chunks...").fill("no-matching-phrase");
    await page.getByPlaceholder("Search chunks...").press("Enter");
    // Then the URL records the query and the chunk disappears.
    await expect(page).toHaveURL(/q=no-matching-phrase/);
    await expect(page.getByText("Searchable note", { exact: true })).toHaveCount(0);
});

test("the note filter excludes a document", async ({ page, network }) => {
    // Given a note and a document.
    await seed(page, network.origin, "Note filter item", { type: "note" });
    await seed(page, network.origin, "Document filter item", { type: "document" });
    await page.goto("/chunks");
    await page.waitForLoadState("networkidle");
    // When the note type is selected.
    await page.getByRole("button", { name: "Filters" }).click();
    await page.getByRole("button", { name: "note", exact: true }).click();
    // Then only the note remains.
    await expect(page.getByText("Note filter item", { exact: true })).toBeVisible();
    await expect(page.getByText("Document filter item", { exact: true })).toHaveCount(0);
});

test("a tag filter excludes an untagged chunk", async ({ page, network }) => {
    // Given tagged and untagged chunks.
    await seed(page, network.origin, "Tagged browse item", { tags: ["browse-tag"] });
    await seed(page, network.origin, "Plain browse item");
    await page.goto("/chunks");
    await page.waitForLoadState("networkidle");
    // When the tag is selected.
    await page.getByRole("button", { name: "Filters" }).click();
    await page.getByText("browse-tag", { exact: true }).click();
    // Then only the tagged chunk remains.
    await expect(page.getByText("Tagged browse item", { exact: true })).toBeVisible();
    await expect(page.getByText("Plain browse item", { exact: true })).toHaveCount(0);
});

test("two selected tags include chunks carrying either tag", async ({ page, network }) => {
    // Given two differently tagged chunks and an untagged chunk.
    await seed(page, network.origin, "Red browse item", { tags: ["red-browse"] });
    await seed(page, network.origin, "Blue browse item", { tags: ["blue-browse"] });
    await seed(page, network.origin, "Plain browse item");
    await page.goto("/chunks");
    await page.waitForLoadState("networkidle");
    // When both tags are selected.
    await page.getByRole("button", { name: "Filters" }).click();
    await page.getByText("red-browse", { exact: true }).click();
    await page.getByText("blue-browse", { exact: true }).click();
    // Then the two tagged chunks remain and the plain chunk is excluded.
    await expect(page.getByText("Red browse item", { exact: true })).toBeVisible();
    await expect(page.getByText("Blue browse item", { exact: true })).toBeVisible();
    await expect(page.getByText("Plain browse item", { exact: true })).toHaveCount(0);
});

test("alphabetical sorting orders chunk titles", async ({ page, network }) => {
    // Given titles created in reverse alphabetical order.
    await seed(page, network.origin, "Zeta browse item");
    await seed(page, network.origin, "Alpha browse item");
    await page.goto("/chunks");
    await page.waitForLoadState("networkidle");
    // When the user chooses alphabetical sorting.
    await page.getByRole("button", { name: "Filters" }).click();
    await page
        .locator("select")
        .filter({ has: page.locator('option[value="alpha"]') })
        .selectOption("alpha");
    // Then Alpha precedes Zeta in the rendered list.
    await expect(page).toHaveURL(/sort=alpha/);
    const links = page.getByRole("link", { name: /(?:Alpha|Zeta) browse item/ });
    await expect(links).toHaveCount(2);
    expect((await links.allTextContents())[0]).toContain("Alpha browse item");
});

test("oldest-first sorting survives reload", async ({ page, network }) => {
    // Given two chunks created sequentially.
    await seed(page, network.origin, "First browse item");
    await seed(page, network.origin, "Second browse item");
    await page.goto("/chunks");
    await page.waitForLoadState("networkidle");
    // When oldest-first sorting is selected and the page reloads.
    await page.getByRole("button", { name: "Filters" }).click();
    await page
        .locator("select")
        .filter({ has: page.locator('option[value="oldest"]') })
        .selectOption("oldest");
    await page.reload();
    // Then the selected sort and creation order persist.
    await expect(page).toHaveURL(/sort=oldest/);
    const links = page.getByRole("link", { name: /(?:First|Second) browse item/ });
    await expect(links).toHaveCount(2);
    expect((await links.allTextContents())[0]).toContain("First browse item");
});

test("grouping by type shows note and document sections", async ({ page, network }) => {
    // Given chunks with different types.
    await seed(page, network.origin, "Group note", { type: "note" });
    await seed(page, network.origin, "Group document", { type: "document" });
    await page.goto("/chunks");
    await page.waitForLoadState("networkidle");
    // When grouping by type is selected.
    await page
        .locator("select")
        .filter({ has: page.locator('option[value="type"]') })
        .first()
        .selectOption("type");
    // Then the grouping is URL-backed and both chunks remain available.
    await expect(page).toHaveURL(/group=type/);
    await page.getByRole("button", { name: "note (1)" }).click();
    await page.getByRole("button", { name: "document (1)" }).click();
    await expect(page.getByText("Group note", { exact: true })).toBeVisible();
    await expect(page.getByText("Group document", { exact: true })).toBeVisible();
});

test("grouping by origin keeps human chunks visible", async ({ page, network }) => {
    // Given a human-authored chunk.
    await seed(page, network.origin, "Human origin item");
    await page.goto("/chunks");
    await page.waitForLoadState("networkidle");
    // When grouping by origin is selected.
    await page
        .locator("select")
        .filter({ has: page.locator('option[value="origin"]') })
        .first()
        .selectOption("origin");
    // Then the chunk appears in the origin grouping after reload.
    await expect(page).toHaveURL(/group=origin/);
    await page.reload();
    await page.getByRole("button", { name: "human (1)" }).click();
    await expect(page.getByText("Human origin item", { exact: true })).toBeVisible();
});

test("keyboard selection opens the highlighted chunk", async ({ page, network }) => {
    // Given a single chunk in the list.
    const chunk = await seed(page, network.origin, "Keyboard browse item");
    await page.goto("/chunks");
    await page.waitForLoadState("networkidle");
    await expect(page.getByText("Keyboard browse item", { exact: true })).toBeVisible();
    // When the user presses j and Enter outside an input.
    await page.locator("body").press("j");
    await page.locator("body").press("Enter");
    // Then the highlighted chunk opens.
    await expect(page).toHaveURL(new RegExp(`/chunks/${chunk.id}$`));
});

test("browser Back restores the previous chunk query", async ({ page, network }) => {
    // Given two different chunk titles.
    await seed(page, network.origin, "Back first item");
    await seed(page, network.origin, "Back second item");
    await page.goto("/chunks");
    await page.waitForLoadState("networkidle");
    // When the user searches twice and navigates back.
    const search = page.getByPlaceholder("Search chunks...");
    await search.fill("first");
    await search.press("Enter");
    await expect(page).toHaveURL(/q=first/);
    await search.fill("second");
    await search.press("Enter");
    await expect(page).toHaveURL(/q=second/);
    await page.goBack();
    // Then the first query and its result return.
    await expect(page).toHaveURL(/q=first/);
    await expect(search).toHaveValue("first");
    await expect(page.getByText("Back first item", { exact: true })).toBeVisible();
});
