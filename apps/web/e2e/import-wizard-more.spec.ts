import { fileURLToPath } from "node:url";

import type { Page } from "@playwright/test";

import { siteTest as test } from "./support/site-test";
import { expect } from "./support/test";

const folder = fileURLToPath(new URL("./fixtures/import-docs", import.meta.url));

async function openImport(page: Page) {
    await page.goto("/import");
    await page.waitForLoadState("networkidle");
}

async function createSpace(page: Page, origin: string) {
    const response = await page.request.post(`${origin}/api/spaces`, { data: { name: `Wizard space ${crypto.randomUUID().slice(0, 6)}` } });
    expect(response.status()).toBe(201);
    return ((await response.json()) as { id: string }).id;
}

async function prepareWizard(page: Page, origin: string) {
    const spaceId = await createSpace(page, origin);
    await openImport(page);
    await page.getByRole("button", { name: "Wizard", exact: true }).click();
    await page.locator('input[type="file"][webkitdirectory]').setInputFiles(folder);
    await page.locator("select").selectOption(spaceId);
    await expect(page.getByText("2 selected, 0 deselected", { exact: true })).toBeVisible();
    return spaceId;
}

test("Quick import requires a selected space", async ({ page }) => {
    // Given two Markdown files selected in Quick mode.
    await openImport(page);
    await page.locator('input[type="file"][webkitdirectory]').setInputFiles(folder);
    // When no destination space is selected.
    // Then the Import action stays disabled.
    await expect(page.getByRole("button", { name: "Import 2 files" })).toBeDisabled();
});

test("the selected import mode survives reload", async ({ page }) => {
    // Given the import page in Quick mode.
    await openImport(page);
    // When Wizard is selected and the page reloads.
    await page.getByRole("button", { name: "Wizard", exact: true }).click();
    await page.reload();
    // Then the wizard selection is restored.
    await expect(page.getByText("Step 1 of 4", { exact: true })).toBeVisible();
    expect(await page.evaluate(() => localStorage.getItem("import-mode"))).toBe("wizard");
});

test("wizard Preview is disabled before files are selected", async ({ page }) => {
    // Given the wizard's empty first step.
    await openImport(page);
    await page.getByRole("button", { name: "Wizard", exact: true }).click();
    // When no folder has been chosen.
    // Then Preview is unavailable.
    await expect(page.getByRole("button", { name: "Preview →" })).toBeDisabled();
});

test("wizard Preview requires a destination space", async ({ page }) => {
    // Given a selected folder but no space.
    await openImport(page);
    await page.getByRole("button", { name: "Wizard", exact: true }).click();
    await page.locator('input[type="file"][webkitdirectory]').setInputFiles(folder);
    // When the folder is ready to preview.
    // Then the wizard still requires a space.
    await expect(page.getByText("2 selected, 0 deselected", { exact: true })).toBeVisible();
    await expect(page.getByRole("button", { name: "Preview →" })).toBeDisabled();
});

test("deselecting all wizard files disables Preview", async ({ page, network }) => {
    // Given a selected space and two selected files.
    await prepareWizard(page, network.origin);
    // When Deselect all is clicked.
    await page.getByRole("button", { name: "Deselect all" }).click();
    // Then Preview is disabled and both files are deselected.
    await expect(page.getByText("0 selected, 2 deselected", { exact: true })).toBeVisible();
    await expect(page.getByRole("button", { name: "Preview →" })).toBeDisabled();
});

test("Select all restores wizard file selection", async ({ page, network }) => {
    // Given a prepared wizard after all files were deselected.
    await prepareWizard(page, network.origin);
    await page.getByRole("button", { name: "Deselect all" }).click();
    // When Select all is clicked.
    await page.getByRole("button", { name: "Select all", exact: true }).click();
    // Then both Markdown files are selected and Preview is enabled.
    await expect(page.getByText("2 selected, 0 deselected", { exact: true })).toBeVisible();
    await expect(page.getByRole("button", { name: "Preview →" })).toBeEnabled();
});

test("wizard Back preserves the selected folder and space", async ({ page, network }) => {
    // Given a prepared wizard.
    const spaceId = await prepareWizard(page, network.origin);
    // When the user goes to preview and back.
    await page.getByRole("button", { name: "Preview →" }).click();
    await expect(page.getByText("Step 2 of 4", { exact: true })).toBeVisible();
    await page.getByRole("button", { name: "Back" }).click();
    // Then both files and the destination remain selected.
    await expect(page.getByText("2 selected, 0 deselected", { exact: true })).toBeVisible();
    await expect(page.locator("select")).toHaveValue(spaceId);
});

test("wizard preview loads parsed file titles", async ({ page, network }) => {
    // Given a prepared wizard.
    await prepareWizard(page, network.origin);
    // When Preview is opened.
    await network.perform({ method: "POST", path: "/api/chunks/import-docs/preview", status: 200 }, () =>
        page.getByRole("button", { name: "Preview →" }).click()
    );
    // Then the parsed guide title appears in the preview panel.
    await expect(page.getByRole("heading", { name: "Imported Guide" })).toBeVisible();
});

test("a title override appears on the wizard Review step", async ({ page, network }) => {
    // Given a wizard preview of two Markdown files.
    await prepareWizard(page, network.origin);
    await page.getByRole("button", { name: "Preview →" }).click();
    await expect(page.getByRole("heading", { name: "Imported Guide" })).toBeVisible();
    // When the first file title is overridden and Review is opened.
    await page
        .getByRole("heading", { name: "Imported Guide" })
        .locator("xpath=..")
        .locator('input[value="Imported Guide"]')
        .fill("Overridden guide");
    await page.getByRole("button", { name: "Review →" }).click();
    // Then Review shows the override.
    await expect(page.getByText("Overridden guide", { exact: true })).toBeVisible();
});

test("wizard Review counts two new chunks", async ({ page, network }) => {
    // Given a wizard preview of two Markdown files.
    await prepareWizard(page, network.origin);
    await page.getByRole("button", { name: "Preview →" }).click();
    await expect(page.getByRole("heading", { name: "Imported Guide" })).toBeVisible();
    // When Review is opened.
    await page.getByRole("button", { name: "Review →" }).click();
    // Then both files are classified as new chunks.
    await expect(page.getByRole("button", { name: "New chunks (2)" })).toBeVisible();
    await expect(page.getByText("Step 3 of 4", { exact: true })).toBeVisible();
});
