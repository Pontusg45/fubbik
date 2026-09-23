import { defineForm, expect, test } from "../support/test";

test.beforeEach(async ({ page }) => {
    await page.goto("/");
});

test("menus compose checkbox and radio items and can reopen after an action", async ({ page, ui }) => {
    // Given a dropdown with checkbox, radio and action items.
    const dropdown = ui.dropdownMenu("Chunk actions");
    const menu = await dropdown.open();
    // When the checkbox is enabled twice.
    await menu.checkbox("Show archived").set(true);
    await menu.checkbox("Show archived").set(true);
    // Then it remains checked.
    await menu.checkbox("Show archived").expectChecked();
    // When Grid view is chosen repeatedly.
    await menu.radio("Grid view").choose();
    await menu.radio("Grid view").choose();
    // Then Grid view stays selected.
    await menu.radio("Grid view").expectChecked();
    await expect(menu.root.getByRole("menuitem", { name: "Delete chunk" })).toBeDisabled();
    // When Export chunk is activated.
    await menu.choose("Export chunk");
    // Then the menu closes, exports, and restores trigger focus.
    await menu.expectHidden();
    await expect(ui.button("Chunk actions").root).toBeFocused();
    await expect(page.getByLabel("Menu settings")).toHaveText(JSON.stringify({ archived: true, view: "grid", action: "Exported" }));
    // When the menu is reopened and its checkbox cleared.
    const reopened = await dropdown.open();
    await reopened.checkbox("Show archived").set(false);
    // Then the checkbox is unchecked.
    await reopened.checkbox("Show archived").expectChecked(false);
    await dropdown.choose("Export chunk");
    await reopened.expectHidden();
});

test("popovers resolve outside a local scope and reopen after Escape", async ({ page, ui }) => {
    // Given a popover trigger inside a local region.
    const scoped = ui.within(page.getByRole("region", { name: "Overlay contracts" }));
    const popover = scoped.popover("Edit summary");
    // When the portaled popover is opened and edited.
    const content = await popover.open();
    const form = defineForm({ title: ui.within(content.root).input("Title") });
    await form.fill({ title: "Summary" });
    // Then opening it again reuses its visible content and preserves the edit.
    await (await popover.open()).expectVisible();
    await form.fields.title.expectValue("Summary");
    // When the popover is dismissed.
    await popover.close();
    // Then focus returns to the local trigger.
    await expect(scoped.button("Edit summary").root).toBeFocused();
    await (await popover.open()).expectVisible();
    await popover.close();
    await content.expectHidden();
});

test("explicit popover content works inside a frame", async ({ page, ui, baseURL }) => {
    // Given an explicit popover content binding inside a frame.
    await page.setContent(`<iframe title="Embedded Fubbik" src="${baseURL}/" style="width:1000px;height:700px"></iframe>`);
    const frame = page.frameLocator("iframe");
    const popover = ui.within(frame).popover("Edit summary", {
        content: frame.getByRole("dialog", { name: "Chunk summary" })
    });
    // When the frame-local popover is opened and edited.
    const content = await popover.open();
    await ui.within(content.root).input("Title").fill("In frame");
    // Then the frame-local form reflects the edit.
    await ui.within(content.root).input("Title").expectValue("In frame");
    await popover.close();
    await expect(frame.getByRole("button", { name: "Edit summary" })).toBeFocused();
});

test("sheets support scoped forms, Escape dismissal and the default close button", async ({ ui }) => {
    // Given a sheet and its trigger.
    const sheet = ui.sheet("Chunk inspector");
    const trigger = ui.button("Open inspector");
    // When the sheet is opened and its scoped form edited.
    await sheet.open(trigger);
    const form = defineForm({ title: ui.within(sheet.root).input("Title") });
    await form.fill({ title: "Inspected chunk" });
    // Then the sheet field reflects the edit.
    await form.fields.title.expectValue("Inspected chunk");
    // When Escape dismisses the sheet.
    await sheet.dismiss();
    // Then focus returns to the sheet trigger.
    await expect(trigger.root).toBeFocused();
    await sheet.open(trigger);
    await sheet.close();
    await expect(trigger.root).toBeFocused();
});

test("disclosures expand and collapse idempotently for collapsibles and accordions", async ({ page, ui }) => {
    // Given collapsible and accordion disclosures.
    const context = ui.disclosure("Decision context");
    const panel = page.getByText("Considered PostgreSQL and SQLite.", { exact: true });
    // When the collapsible is collapsed and expanded repeatedly.
    await context.collapse();
    await context.expand();
    await context.expand();
    // Then repeated expansion leaves the panel visible.
    await expect(panel).toBeVisible();
    await context.collapse();
    await context.collapse();
    await expect(panel).toBeHidden();
    const rationale = ui.disclosure("Rationale");
    // When the accordion is expanded repeatedly.
    await rationale.expand();
    await rationale.expand();
    // Then the rationale is visible.
    await expect(page.getByText("Transactions keep chunk writes atomic.", { exact: true })).toBeVisible();
    await rationale.collapse();
    await expect(page.getByText("Transactions keep chunk writes atomic.", { exact: true })).toBeHidden();
});

test("confirmation dialogs distinguish cancellation from confirmation", async ({ page, ui }) => {
    // Given a confirmation dialog for a preserved chunk.
    const dialog = ui.dialog("Delete fixture chunk?");
    const trigger = ui.button("Delete fixture chunk");
    // When deletion is requested and cancelled.
    await dialog.open(trigger);
    await dialog.close("Cancel");
    // Then the chunk is preserved.
    await expect(page.getByLabel("Deletion result")).toHaveText("Preserved");
    await dialog.open(trigger);
    // When deletion is confirmed.
    await dialog.button("Delete").click();
    // Then the dialog closes and the deletion result changes.
    await dialog.expectHidden();
    await expect(page.getByLabel("Deletion result")).toHaveText("Deleted");
});
