import { fileURLToPath } from "node:url";

import { siteTest as test } from "./support/site-test";
import { expect } from "./support/test";

const folder = fileURLToPath(new URL("./fixtures/import-docs", import.meta.url));

async function prepareImport(page: import("@playwright/test").Page, screens: import("./support/test").FubbikFixtures["screens"]) {
    const spaceId = await screens.spaces.create(`Import space ${crypto.randomUUID().slice(0, 8)}`);
    await page.goto("/import");
    await page.waitForLoadState("networkidle");
    await page.getByRole("button", { name: "Quick", exact: true }).click();
    await page.locator('input[type="file"][webkitdirectory]').setInputFiles(folder);
    await page.locator("select").selectOption(spaceId);
    await expect(page.getByText("Imported Guide", { exact: true })).toBeVisible();
    await expect(page.getByText("Imported Notes", { exact: true })).toBeVisible();
    await expect(page.getByText("ignored.txt", { exact: true })).toHaveCount(0);
    return spaceId;
}

test("Quick import previews Markdown files and persists the resulting chunks", async ({ page, screens, network }) => {
    // Given a space and a folder with two Markdown files and one ignored text file.
    await prepareImport(page, screens);
    // When the selected files are imported.
    const response = await network.perform({ method: "POST", path: "/api/chunks/import-docs", status: 200 }, () =>
        page.getByRole("button", { name: "Import 2 files" }).click()
    );
    // Then the API and results view report two created chunks.
    const result = (await response.json()) as { created: number; skipped: number; errors: unknown[] };
    expect(result).toMatchObject({ created: 2, skipped: 0, errors: [] });
    await expect(page.getByText("Created: 2", { exact: true })).toBeVisible();
    await page.goto("/chunks");
    await expect(page.getByRole("link", { name: /^Imported Guide — Introduction/ })).toBeVisible();
    await expect(page.getByRole("link", { name: /^Imported Notes — Introduction/ })).toBeVisible();
    await page.reload();
    await expect(page.getByRole("link", { name: /^Imported Guide — Introduction/ })).toBeVisible();
    await expect(page.getByRole("link", { name: /^Imported Notes — Introduction/ })).toBeVisible();
});

test("an import failure retains the file selection for a successful retry", async ({ page, screens, network }) => {
    // Given a prepared import with two selected Markdown files.
    await prepareImport(page, screens);
    const endpoint = { method: "POST", path: "/api/chunks/import-docs" } as const;
    // When the server rejects the first import.
    await network.withFailure(endpoint, async () => {
        await network.perform({ ...endpoint, status: 500 }, () => page.getByRole("button", { name: "Import 2 files" }).click());
    });
    // Then the selection remains available and no result is reported.
    await expect(page.getByText("Failed to import docs", { exact: true })).toBeVisible();
    await expect(page.getByRole("button", { name: "Import 2 files" })).toBeEnabled();
    await expect(page.getByText("Import Results", { exact: true })).toHaveCount(0);
    // When the user retries, then both chunks persist.
    await network.perform({ ...endpoint, status: 200 }, () => page.getByRole("button", { name: "Import 2 files" }).click());
    await expect(page.getByText("Created: 2", { exact: true })).toBeVisible();
    await page.goto("/chunks");
    await expect(page.getByRole("link", { name: /^Imported Guide — Introduction/ })).toBeVisible();
});
