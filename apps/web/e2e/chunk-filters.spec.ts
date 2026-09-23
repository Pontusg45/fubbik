import { expect, test, testAccount } from "./support/test";

test("text, type and tag filters narrow chunks and survive a reload", async ({ page, screens }) => {
    // Given distinct note, document and tagged chunks.
    await screens.auth.signUp(testAccount());
    const note = `Filter note ${crypto.randomUUID().slice(0, 8)}`;
    const document = `Filter document ${crypto.randomUUID().slice(0, 8)}`;
    const tag = `filter-${crypto.randomUUID().slice(0, 8)}`;
    await screens.chunks.createChunk({ title: note, content: "Shared search phrase." });
    await screens.chunks.openNew();
    await screens.chunks.form.fill({ title: document, content: "Shared search phrase." });
    await screens.chunks.type.choose("document");
    await screens.chunks.tags.add(tag);
    await screens.chunks.createAndOpen();
    await page.goto("/chunks");
    await expect(page.getByText(note, { exact: true })).toBeVisible();
    await expect(page.getByText(document, { exact: true })).toBeVisible();
    // When the user searches and applies the document type and unique tag.
    const search = page.getByPlaceholder("Search chunks...", { exact: true });
    await search.fill("Filter");
    await search.press("Enter");
    await expect(page).toHaveURL(/q=Filter/);
    await page.getByRole("button", { name: "Filters" }).click();
    await page.getByRole("button", { name: "document", exact: true }).click();
    await page.getByText(tag, { exact: true }).click();
    // Then only the tagged document is shown and the URL stores the filters.
    await expect(page).toHaveURL(/type=document/);
    await expect(page).toHaveURL(new RegExp(`tags=${tag}`));
    await expect(page.getByText(document, { exact: true })).toBeVisible();
    await expect(page.getByText(note, { exact: true })).toHaveCount(0);
    await page.reload();
    await expect(page.getByText(document, { exact: true })).toBeVisible();
    await expect(page.getByText(note, { exact: true })).toHaveCount(0);
    // When the user clears all filters, then both chunks return.
    await page.getByRole("button", { name: "Filters" }).click();
    await page.getByRole("dialog").getByRole("button", { name: "Clear all" }).click();
    await expect(page.getByText(note, { exact: true })).toBeVisible();
    await expect(page.getByText(document, { exact: true })).toBeVisible();
});
