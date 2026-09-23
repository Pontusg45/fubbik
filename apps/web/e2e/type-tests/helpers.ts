import type { Page } from "@playwright/test";

import type { ApiTraffic } from "../support/network";
import type { ChunkEditor } from "../support/screens/chunks";
import { defineForm, FubbikUI } from "../support/ui";

// Compiled by check-types:e2e, never executed. Each expected error is a public API contract.
export async function helperTypeContracts(page: Page) {
    // Given typed UI bindings for the browser page.
    const ui = new FubbikUI(page);
    const type = ui.select("Chunk type", { options: { note: "Note", document: "Document" } });
    // When helper calls are type-checked with valid and invalid values.
    // Then valid calls compile and each expected error enforces a public contract.
    await type.choose("note");
    // @ts-expect-error Unknown option keys must not compile.
    await type.choose("task");
    // @ts-expect-error Single-select has no multiselect operation.
    await type.setSelected("note", true);
    const native = ui.nativeSelect("Type", { note: "note", document: "document" });
    await defineForm({ native }).fill({ native: "document" });
    // @ts-expect-error Native selects preserve finite option keys.
    await native.choose("missing");
    // @ts-expect-error Native selects accept one value.
    await native.set(["note"]);
    const tags = ui.tagInput("Tags", { chip: value => page.getByText(value) });
    // @ts-expect-error Tag actions do not promise replacement form semantics.
    defineForm({ tags });
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
    const menu = await ui.dropdownMenu("Chunk actions").open();
    await menu.checkbox("Show archived").set(true);
    await menu.radio("Grid view").choose();
    // @ts-expect-error Menu radio items cannot be unchecked individually.
    await menu.radio("Grid view").set(false);
    // @ts-expect-error Menu actions are not writable form fields.
    defineForm({ action: menu });
    const popover = ui.popover("Edit summary");
    const content = await popover.open();
    await ui.within(content.root).input("Title").fill("Summary");
    // @ts-expect-error Resolve overlay content by opening it first.
    ui.within(popover.root);
    // @ts-expect-error Disclosures expose expansion, not field values.
    defineForm({ context: ui.disclosure("Decision context") });
    await ui.sheet("Chunk inspector").open(ui.button("Open inspector"));
}

export async function workflowTypeContracts(network: ApiTraffic, chunks: ChunkEditor) {
    // Given typed workflow and API helpers.
    const action = async () => {};
    // When valid and invalid workflows are type-checked.
    // Then only complete drafts and valid API endpoints compile.
    await chunks.createChunk({ title: "Decision", content: "Use PostgreSQL." });
    // @ts-expect-error A new chunk requires content as well as a title.
    await chunks.createChunk({ title: "Incomplete" });
    await network.perform({ method: "POST", path: "/api/chunks", status: 201 }, action);
    // @ts-expect-error A response status must be explicit.
    await network.perform({ method: "POST", path: "/api/chunks" }, action);
    // @ts-expect-error API traffic bindings use absolute API paths.
    network.record({ method: "GET", path: "/chunks" });
    // @ts-expect-error Unsupported methods are rejected.
    network.record({ method: "REMOVE", path: "/api/chunks" });
}
