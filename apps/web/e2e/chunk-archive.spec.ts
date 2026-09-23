import { expect, test, testAccount } from "./support/test";

const draft = { title: "Archive workflow chunk", content: "Restore this persisted content." };

test("archiving removes a chunk from active results and restoring brings it back", async ({ page, screens, network }) => {
    // Given an authenticated user with an active chunk.
    await screens.auth.signUp(testAccount());
    const chunk = await screens.chunks.createChunk(draft);
    // When the user archives it from its detail page.
    await network.perform({ method: "POST", path: `${chunk.path}/archive`, status: 200 }, () =>
        page
            .getByRole("button", { name: "More actions" })
            .click()
            .then(() => page.getByRole("menuitem", { name: "Archive" }).click())
    );
    // Then the active list no longer contains it, including after reload.
    await expect(page).toHaveURL(/\/chunks\/?$/);
    await page.reload();
    await expect(page.getByText(draft.title, { exact: true })).toHaveCount(0);
    // When the user opens the archive and restores the chunk.
    await page.goto("/chunks/archived");
    await expect(page.getByText(draft.title, { exact: true })).toBeVisible();
    await network.perform({ method: "POST", path: `${chunk.path}/restore`, status: 200 }, () =>
        page.getByRole("button", { name: "Restore" }).click()
    );
    // Then the archive is empty and the active list contains the original chunk.
    await page.reload();
    await expect(page.getByText(draft.title, { exact: true })).toHaveCount(0);
    await page.goto("/chunks");
    await expect(page.getByText(draft.title, { exact: true })).toBeVisible();
    await page.goto(chunk.url);
    await screens.chunks.expectDetails(draft);
});
