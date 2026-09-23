import { defineForm, expect, test } from "../support/test";

test.beforeEach(async ({ page }) => {
    await page.goto("/");
});

test("invalid multiselect values fail before any field changes", async ({ page, ui }) => {
    // Given a form with a title and multiselect.
    const settings = defineForm({
        title: ui.input(page.locator("#title")),
        types: ui.multiSelect("Included types", { options: { note: "Note", document: "Document" } })
    });
    // When invalid multiselect values are submitted.
    await expect(settings.fill({ title: "Do not write", types: ["note", "note"] })).rejects.toThrow("must be unique");
    await expect(settings.fill({ title: "Do not write", types: ["unknown"] } as never)).rejects.toThrow("Unknown option key");
    await expect(settings.patch({ types: "note" } as never)).rejects.toThrow("requires an array");
    // Then no field changes and no overlay opens.
    await settings.fields.title.expectValue("");
    await expect(settings.fields.types.root).not.toHaveAttribute("aria-expanded", "true");
});

test("disabled menu actions expose their disabled state and reject activation", async ({ page, ui }) => {
    // Given an open menu with a disabled delete action.
    const menu = await ui.dropdownMenu("Chunk actions").open();
    const disabled = ui.button(menu.root.getByRole("menuitem", { name: "Delete chunk", exact: true }));
    await disabled.expectDisabled();
    // When activation of the disabled action is attempted.
    await expect(disabled.root.click({ timeout: 500 })).rejects.toThrow("not enabled");
    // Then the menu stays open and its state is unchanged.
    await menu.expectVisible();
    await expect(page.getByLabel("Menu settings")).toHaveText(JSON.stringify({ archived: false, view: "list", action: "None" }));
});

test("duplicate row keys fail with a useful diagnostic", async ({ page, ui }) => {
    // Given a table containing duplicate row keys.
    // Deliberately corrupt a valid component fixture to exercise ambiguous application data.
    await page
        .getByRole("table", { name: "Chunks" })
        .locator("tbody")
        .evaluate(body => {
            body.append(body.firstElementChild!.cloneNode(true));
        });
    const table = ui.table("Chunks", { columns: { title: { label: "Title" }, type: { label: "Type" } } });
    // When a row is resolved by its ambiguous key.
    // Then the helper rejects the ambiguous match with a diagnostic.
    await expect(table.row({ column: "title", value: "Architecture" }).expectCellText("type", "Note")).rejects.toThrow(
        "Row key must match exactly one rendered row"
    );
});

test("ambiguous explicit overlay bindings fail rather than selecting the first match", async ({ page, ui }) => {
    // Given an explicit overlay binding that matches multiple sections.
    const popover = ui.popover("Edit summary", { content: page.locator("section") });
    // When the popover is opened.
    // Then the helper rejects the ambiguous content binding.
    await expect(popover.open()).rejects.toThrow("Overlay content must resolve uniquely");
});
