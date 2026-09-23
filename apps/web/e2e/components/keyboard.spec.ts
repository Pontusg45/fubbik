import { expect, test } from "../support/test";

test.beforeEach(async ({ page }) => {
    await page.goto("/");
});

test("Tab moves through fields and Space toggles a checkbox", async ({ page, ui }) => {
    // Given the title field has keyboard focus.
    const title = ui.input(page.locator("#title"));
    await title.root.focus();
    // When Tab advances through the controls.
    await page.keyboard.press("Tab");
    // Then focus moves to the select.
    await expect(ui.select("Chunk type", { options: { note: "Note" } }).root).toBeFocused();
    await page.keyboard.press("Tab");
    const pinned = ui.checkbox("Pinned");
    await expect(pinned.root).toBeFocused();
    // When Space toggles the focused checkbox.
    await page.keyboard.press("Space");
    // Then the checkbox is checked.
    await pinned.expectChecked();
    await page.keyboard.press("Space");
    await pinned.expectChecked(false);
});

test("select supports arrow-key selection and restores focus", async ({ page, ui }) => {
    // Given a select trigger with keyboard focus.
    const select = ui.select("Chunk type", { options: { note: "Note", document: "Document" } });
    await select.root.focus();
    // When the select is opened with Enter.
    await page.keyboard.press("Enter");
    // Then its options become visible.
    await expect(page.getByRole("listbox")).toBeVisible();
    // When the next item is reached with ArrowDown.
    await page.keyboard.press("ArrowDown");
    await page.keyboard.press("Enter");
    // Then Document is selected and focus returns to the select.
    await select.expectValue("document");
    await expect(select.root).toBeFocused();
});

test("menu supports arrow navigation and Escape restores trigger focus", async ({ page, ui }) => {
    // Given a menu trigger with keyboard focus.
    const trigger = ui.button("Chunk actions");
    await trigger.root.focus();
    // When the menu is opened with Enter.
    await page.keyboard.press("Enter");
    // Then the menu becomes visible.
    await expect(page.getByRole("menu")).toBeVisible();
    await page.keyboard.press("ArrowDown");
    await expect(page.getByRole("menuitemradio", { name: "List view" })).toBeFocused();
    // When Escape dismisses the overlay.
    await page.keyboard.press("Escape");
    // Then the menu closes and focus returns to its trigger.
    await expect(page.getByRole("menu")).toBeHidden();
    await expect(trigger.root).toBeFocused();
});

for (const triggerName of ["Edit details", "Open inspector", "Edit summary"]) {
    test(`${triggerName} opens with Enter and dismisses with Escape`, async ({ page, ui }) => {
        // Given an overlay trigger with keyboard focus.
        const trigger = ui.button(triggerName);
        await trigger.root.focus();
        // When Enter activates the trigger.
        await page.keyboard.press("Enter");
        // Then the overlay becomes visible.
        await expect(page.getByRole("dialog")).toBeVisible();
        await page.keyboard.press("Escape");
        // Then the dialog closes and focus returns to its trigger.
        await expect(page.getByRole("dialog")).toBeHidden();
        await expect(trigger.root).toBeFocused();
    });
}
