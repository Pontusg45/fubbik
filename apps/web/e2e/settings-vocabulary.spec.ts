import { expect, test, testAccount } from "./support/test";

test("vocabulary settings compose both editable catalogs", async ({ page, screens }) => {
    // Given an authenticated user on vocabulary settings.
    await screens.auth.signUp(testAccount());
    await page.goto("/settings/vocabulary");
    await page.waitForLoadState("networkidle");

    // When each catalog's add form is opened.
    await page.getByRole("button", { name: "Add type" }).click();
    await page.getByRole("button", { name: "Add relation" }).click();

    // Then both forms remain available on the same page.
    await expect(page.getByRole("heading", { name: "Chunk types" })).toBeVisible();
    await expect(page.getByRole("heading", { name: "Connection relations" })).toBeVisible();
    await expect(page.getByText("Slug (id)")).toHaveCount(2);
});
