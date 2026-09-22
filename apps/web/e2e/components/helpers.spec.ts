import { defineForm, expect, test } from "../support/test";

const typeOptions = { note: "Note", document: "Document" } as const;
test.beforeEach(async ({ page }) => {
    await page.goto("/");
});

test("typed forms compose Fubbik inputs, portaled selects, checkboxes and switches", async ({ page, ui }) => {
    const scope = ui.within(page.getByRole("form", { name: "Chunk settings" }));
    const settings = defineForm({
        title: scope.input("Title"),
        type: scope.select("Chunk type", { options: typeOptions }),
        pinned: scope.checkbox("Pinned"),
        notify: scope.switch("Notifications")
    });
    await settings.fill({ title: "Architecture", type: "document", pinned: true, notify: true });
    await expect(page.getByLabel("Saved settings")).toHaveText(
        JSON.stringify({ title: "Architecture", type: "document", pinned: true, notify: true })
    );
    await settings.patch({ pinned: false });
    await settings.fields.pinned.expectChecked(false);
    await settings.fields.notify.expectChecked();
    await settings.fields.type.expectValue("document");
    await ui
        .within(page.getByRole("region", { name: "Other form" }))
        .input("Title")
        .expectValue("Untouched");
});

test("form validation rejects unknown fields and invalid values before changing the page", async ({ page, ui }) => {
    const settings = defineForm({ title: ui.input(page.locator("#title")), pinned: ui.checkbox("Pinned") });
    // Deliberately bypass the type checker to exercise the untyped caller contract.
    await expect(settings.fill({ title: "Should not be written", pinned: "yes" } as never)).rejects.toThrow("boolean");
    await expect(settings.patch({ title: "Should not be written", typo: true } as never)).rejects.toThrow("Unknown form field");
    await expect(settings.fill({ title: "Incomplete" } as never)).rejects.toThrow("every form field");
    await settings.fields.title.expectValue("");
    await settings.fields.pinned.expectChecked(false);
});

test("single and multiple choices enforce their own semantics", async ({ ui }) => {
    const types = ui.multiSelect("Included types", { options: typeOptions });
    await types.set(["note", "document"]);
    const choices = defineForm({ types, visibility: ui.radioGroup("Visibility", { private: "Private", shared: "Shared" }) });
    await choices.fill({ types: ["note"], visibility: "shared" });
    await choices.fields.visibility.expectValue("shared");
    await types.expectSelected("note");
    await types.expectSelected("document", false);
    await types.overlay.close();
    await ui.radioGroup("Visibility", { private: "Private", shared: "Shared" }).choose("shared");
    await ui.radioGroup("Visibility", { private: "Private", shared: "Shared" }).expectValue("shared");
    await ui.checkbox("Partial selection").expectMixed();
    await expect(ui.select("Chunk type", { options: typeOptions }).choose("wrong" as never)).rejects.toThrow("Unknown option key");
});

test("typed table columns support sorting, row selection and direct cell assertions", async ({ ui, page }) => {
    const chunks = ui.table("Chunks", { columns: { title: { label: "Title", sortable: true }, type: { label: "Type" } } });
    await chunks.sortBy("title", "descending");
    await expect(chunks.root.locator("tbody tr").first()).toContainText("Release checklist");
    const row = chunks.row({ column: "title", value: "Architecture" });
    await row.expectCellText("type", "Note");
    await row.select();
    await row.expectSelected();
    await chunks.expectRowCount(2);
    await expect(page.getByRole("columnheader", { name: "Title", exact: true })).toHaveAttribute("aria-sort", "descending");
});

test("dialogs compose buttons and scoped forms and restore trigger focus", async ({ page, ui }) => {
    const dialog = ui.dialog("Chunk details");
    await dialog.open(ui.button("Edit details"));
    const details = defineForm({ title: ui.within(dialog.root).input("Title") });
    await details.fill({ title: "Decision record" });
    await details.fields.title.expectValue("Decision record");
    await dialog.close("Save");
    await expect(ui.button("Edit details").root).toBeFocused();
    await expect(page.getByRole("dialog")).toHaveCount(0);
});

test("overlay resolution stays inside a frame even when labels repeat outside it", async ({ page, ui }) => {
    await page.setContent(
        '<iframe title="Embedded Fubbik" src="http://127.0.0.1:4178/" style="width:1000px;height:700px"></iframe><button role="combobox" aria-label="Chunk type">Unrelated</button>'
    );
    const frame = page.frameLocator("iframe");
    const select = ui.within(frame).select("Chunk type", { options: typeOptions });
    await select.choose("document");
    await select.expectValue("document");
});
