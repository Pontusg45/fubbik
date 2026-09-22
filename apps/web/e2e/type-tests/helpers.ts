import type { Page } from "@playwright/test";

import { defineForm, FubbikUI } from "../support/ui";

// Compiled by check-types:e2e, never executed. Each expected error is a public API contract.
export async function helperTypeContracts(page: Page) {
    const ui = new FubbikUI(page);
    const type = ui.select("Chunk type", { options: { note: "Note", document: "Document" } });
    await type.choose("note");
    // @ts-expect-error Unknown option keys must not compile.
    await type.choose("task");
    // @ts-expect-error Single-select has no multiselect operation.
    await type.setSelected("note", true);
    const multiple = ui.multiSelect("Types", { options: { note: "Note", document: "Document" } });
    await multiple.set(["note", "document"]);
    // @ts-expect-error Multiselect requires an array.
    await multiple.set("note");
    // @ts-expect-error Switch cannot represent a mixed state.
    await ui.switch("Notify").expectMixed();
    // @ts-expect-error Presentation helpers have no disabled-state API.
    await ui.card(page.locator("article")).expectDisabled();
    const form = defineForm({ title: ui.input("Title"), type, pinned: ui.checkbox("Pinned") });
    await form.fill({ title: "Architecture", type: "note", pinned: true });
    await form.patch({ pinned: false });
    // @ts-expect-error Boolean controls reject string values.
    await form.patch({ pinned: "yes" });
    // @ts-expect-error Literal choice keys are preserved through form inference.
    await form.patch({ type: "missing" });
    // @ts-expect-error fill requires all fields; patch handles partial edits.
    await form.fill({ title: "Incomplete" });
    const unknownFields = { title: "Title", typo: "Oops" };
    // @ts-expect-error Extra keys must fail even when passed via a variable.
    await form.patch(unknownFields);
    const chunks = ui.table("Chunks", { columns: { title: { label: "Title", sortable: true }, type: { label: "Type" } } });
    await chunks.sortBy("title", "ascending");
    await chunks.row({ column: "title", value: "Architecture" }).expectCellText("type", "Note");
    // @ts-expect-error Unknown columns are rejected.
    chunks.row({ column: "titel", value: "Architecture" });
    // @ts-expect-error Only explicitly sortable columns can be sorted.
    await chunks.sortBy("type", "ascending");
    // @ts-expect-error Optional options cannot be explicitly undefined under exactOptionalPropertyTypes.
    ui.select("Type", { options: { note: "Note" }, content: undefined });
}
