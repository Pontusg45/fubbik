import { siteTest as test } from "./support/site-test";
import { expect } from "./support/test";

test("a 200-character chunk title can be saved", async ({ screens }) => {
    // Given a title at the editor's maximum length.
    const draft = { title: "T".repeat(200), content: "At the boundary." };
    // When the user creates the chunk.
    await screens.chunks.createChunk(draft);
    // Then the full title survives a reload.
    await screens.chunks.reloadAndExpect(draft);
});

test("a 201-character title is rejected before a write", async ({ page, screens, network }) => {
    // Given a title one character over the limit.
    await screens.chunks.openNew();
    await screens.chunks.form.fill({ title: "T".repeat(201), content: "Boundary case." });
    const writes = network.record({ method: "POST", path: "/api/chunks" });
    // When the user submits it.
    await screens.chunks.create();
    // Then validation blocks the API request.
    await expect(page.getByText("Title must be 200 characters or less", { exact: true })).toBeVisible();
    expect(writes.requests).toHaveLength(0);
});

test("an overlong content body is rejected before a write", async ({ page, screens, network }) => {
    // Given content above the editor's 50,000-character limit.
    await screens.chunks.openNew();
    await screens.chunks.form.fill({ title: "Large draft", content: "X".repeat(50_001) });
    const writes = network.record({ method: "POST", path: "/api/chunks" });
    // When the user submits it.
    await screens.chunks.create();
    // Then the draft remains and no chunk is created.
    await expect(page.getByText("Content must be 50,000 characters or less", { exact: true })).toBeVisible();
    expect(writes.requests).toHaveLength(0);
});

test("Markdown headings and lists render after saving", async ({ page, screens }) => {
    // Given a chunk containing Markdown structure.
    await screens.chunks.createChunk({ title: "Markdown case", content: "## Heading\n\n- First item\n- Second item" });
    // When the detail page is reloaded.
    await page.reload();
    // Then the content renders as a heading and list items.
    await expect(page.getByRole("heading", { name: "Heading" })).toBeVisible();
    await expect(page.getByRole("listitem").filter({ hasText: "First item" })).toBeVisible();
    await expect(page.getByRole("listitem").filter({ hasText: "Second item" })).toBeVisible();
});

test("editing a chunk type persists the new type", async ({ page, screens, network }) => {
    // Given a saved note chunk.
    const chunk = await screens.chunks.createChunk({ title: "Retype me", content: "Same content." });
    // When its type changes to document.
    await screens.chunks.openEdit();
    await screens.chunks.type.choose("document");
    await screens.chunks.saveAndOpen();
    // Then the API and editor retain document after reload.
    await page.reload();
    const stored = await page.request.get(`${network.origin}${chunk.path}`);
    expect((await stored.json()).chunk.type).toBe("document");
    await screens.chunks.openEdit();
    await screens.chunks.type.expectValue("document");
});

test("removing a tag from an edited chunk persists", async ({ page, screens, network }) => {
    // Given a chunk with a tag.
    await screens.chunks.openNew();
    await screens.chunks.form.fill({ title: "Tagged chunk", content: "Tag removal." });
    await screens.chunks.tags.add("remove-me");
    const chunk = await screens.chunks.createAndOpen();
    // When the tag is removed in the editor.
    await screens.chunks.openEdit();
    await screens.chunks.tags.remove("remove-me");
    await screens.chunks.saveAndOpen();
    // Then it is absent from the stored tag list.
    const stored = await page.request.get(`${network.origin}${chunk.path}`);
    expect((await stored.json()).tags).toEqual([]);
});

test("entering the same tag twice stores it once", async ({ page, screens, network }) => {
    // Given a chunk draft with a normalized tag.
    await screens.chunks.openNew();
    await screens.chunks.form.fill({ title: "Deduplicated tags", content: "One tag." });
    await screens.chunks.tags.add("unique-tag");
    // When the same tag is entered again.
    await page.getByLabel("Tags", { exact: true }).fill("UNIQUE-TAG");
    await page.getByLabel("Tags", { exact: true }).press("Enter");
    const chunk = await screens.chunks.createAndOpen();
    // Then only one normalized tag is saved.
    const stored = await page.request.get(`${network.origin}${chunk.path}`);
    expect((await stored.json()).tags.map((tag: { name: string }) => tag.name)).toEqual(["unique-tag"]);
});

test("decision context changes survive a second edit", async ({ page, screens, network }) => {
    // Given a chunk with decision context.
    await screens.chunks.openNew();
    await screens.chunks.form.fill({ title: "Decision edit", content: "Choose a store." });
    await screens.chunks.setDecisionContext({ alternatives: ["SQLite"], consequences: "Initial consequence." });
    const chunk = await screens.chunks.createAndOpen();
    // When its alternatives and consequences are edited.
    await screens.chunks.openEdit();
    await screens.chunks.decisionContext.fill({ alternatives: "PostgreSQL, MySQL", consequences: "New consequence." });
    await screens.chunks.saveAndOpen();
    // Then the persisted values reflect the edit.
    const stored = await page.request.get(`${network.origin}${chunk.path}`);
    expect((await stored.json()).chunk).toMatchObject({
        alternatives: ["PostgreSQL", "MySQL"],
        consequences: "New consequence."
    });
});

test("a failed create preserves the draft for retry", async ({ page, screens, network }) => {
    // Given a complete chunk draft.
    const draft = { title: "Retry creation", content: "Preserve this text." };
    await screens.chunks.openNew();
    await screens.chunks.form.fill(draft);
    const endpoint = { method: "POST", path: "/api/chunks" } as const;
    // When the first create request fails.
    await network.withFailure(endpoint, () => network.perform({ ...endpoint, status: 500 }, () => screens.chunks.create()));
    // Then the editor retains the draft and a later retry saves it.
    await expect(page.getByText("Failed to create chunk", { exact: true })).toBeVisible();
    await screens.chunks.form.fields.title.expectValue(draft.title);
    await screens.chunks.createAndOpen();
    await screens.chunks.reloadAndExpect(draft);
});

test("an unsaved new-chunk draft returns after navigation", async ({ page, screens }) => {
    // Given an unsaved title and content.
    const draft = { title: "Autosaved draft", content: "Resume after leaving." };
    await screens.chunks.openNew();
    await screens.chunks.form.fill(draft);
    await expect.poll(() => page.evaluate(() => localStorage.getItem("chunk-draft-new"))).toContain(draft.title);
    // When the user leaves and reopens the editor.
    await page.goto("/chunks");
    await screens.chunks.openNew();
    // Then both fields are restored.
    await screens.chunks.form.fields.title.expectValue(draft.title);
    await screens.chunks.form.fields.content.expectValue(draft.content);
});
