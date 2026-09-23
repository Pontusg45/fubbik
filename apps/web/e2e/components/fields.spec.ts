import { defineForm, expect, test } from "../support/test";

test.beforeEach(async ({ page }) => {
    await page.goto("/");
});

test("native selects compose with forms and dispatch changes", async ({ page, ui }) => {
    // Given a native select bound to a typed form.
    const type = ui.nativeSelect("Type", { note: "note", document: "document" });
    // When the form selects Document.
    await defineForm({ type }).fill({ type: "document" });
    // Then the select and application state reflect Document.
    await type.expectValue("document");
    await expect(page.getByLabel("Native type", { exact: true })).toHaveText("document");
    // When the selection is changed back to Note.
    await type.choose("note");
    // Then the native select reflects Note.
    await type.expectValue("note");
});

test("invalid native choices reject before any form mutation", async ({ page, ui }) => {
    // Given a form with a title that must be preserved.
    const title = ui.input(page.locator("#title"));
    await title.fill("Preserved");
    const form = defineForm({ title, type: ui.nativeSelect("Type", { note: "note" }) });
    // When an untyped caller supplies an unknown native option.
    // @ts-expect-error Exercise runtime validation for untyped callers.
    await expect(form.fill({ title: "Changed", type: "missing" })).rejects.toThrow("Unknown option key");
    // Then validation leaves the title unchanged.
    await title.expectValue("Preserved");
});

test("tag inputs normalize, deduplicate and remove exact chips", async ({ page, ui }) => {
    // Given an empty tag input with normalization and exact chip bindings.
    const scope = page.getByRole("region", { name: "Native fields" });
    const tags = ui.within(scope).tagInput("Tags", {
        normalize: value => value.trim().toLowerCase(),
        chip: value => scope.getByText(`${value} ×`, { exact: true })
    });
    // When tags are added, duplicated and selectively removed.
    await tags.add(" Architecture ");
    await tags.add("architecture");
    await tags.add("architecture-extra");
    await tags.remove("ARCHITECTURE");
    // Then only the matching chip is removed.
    await tags.expectTag("architecture", false);
    await tags.expectTag("architecture-extra");
    // Given unsubmitted text in the tag input.
    await tags.input.fill("Preserved");
    // When a blank tag is submitted, then validation rejects it.
    await expect(tags.add(" ")).rejects.toThrow("Tag must not be empty");
    // Then the existing input is preserved.
    await tags.input.expectValue("Preserved");
});
