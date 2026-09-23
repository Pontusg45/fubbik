import { expect, test, testAccount } from "./support/test";

const original = { title: "Workflow chunk", content: "Original persisted content." };

test.beforeEach(async ({ screens }) => {
    await screens.auth.signUp(testAccount({ name: "Chunk Workflow User" }));
});

test("invalid creation preserves input and can be corrected", async ({ page, screens, network }) => {
    // Given an authenticated user and a new chunk editor.
    const writes = network.record({ method: "POST", path: "/api/chunks" });
    await screens.chunks.openNew();
    // When a draft with a blank title is submitted.
    await screens.chunks.form.fill({ title: " ", content: "Keep this draft content." });
    await screens.chunks.create();
    // Then validation preserves the draft without creating a chunk.
    await expect(page.getByText("Title is required", { exact: true })).toBeVisible();
    await screens.chunks.form.fields.content.expectValue("Keep this draft content.");
    expect(writes.requests).toHaveLength(0);
    // When the title is corrected and creation is retried.
    await screens.chunks.form.patch({ title: "Corrected draft" });
    await screens.chunks.createAndOpen();
    // Then the chunk is created and its content survives a reload.
    await screens.chunks.reloadAndExpect({ title: "Corrected draft", content: "Keep this draft content." });
});

test.describe("existing chunk", () => {
    test.beforeEach(async ({ screens }) => {
        await screens.chunks.createChunk(original);
        await screens.chunks.expectDetails(original);
    });

    test("invalid edits and a failed save retain changes for a successful retry", async ({ page, screens, network }) => {
        // Given an authenticated user editing a persisted chunk.
        const chunk = screens.chunks.current();
        const endpoint = { method: "PATCH", path: chunk.path } as const;
        const patches = network.record(endpoint);
        const recovered = { title: "Recovered edit", content: "Unsaved changes to preserve." };
        await screens.chunks.openEdit();
        // When an edit with a blank title is saved.
        await screens.chunks.form.fill({ title: " ", content: recovered.content });
        await screens.chunks.save();
        // Then validation preserves the edit without sending a PATCH.
        await expect(page.getByText("Title is required", { exact: true })).toBeVisible();
        await screens.chunks.form.fields.content.expectValue(recovered.content);
        expect(patches.requests).toHaveLength(0);
        // Given a corrected title and an injected save failure.
        await screens.chunks.form.patch({ title: recovered.title });
        // When the server rejects the save.
        await network.withFailure(endpoint, async () => {
            await network.perform({ ...endpoint, status: 500 }, () => screens.chunks.save());
            // Then the failed save preserves the edit and leaves stored data unchanged.
            await expect(page.getByText("Failed to update chunk", { exact: true })).toBeVisible();
            await expect(page).toHaveURL(`${chunk.url}/edit`);
            await screens.chunks.form.fields.title.expectValue(recovered.title);
            await screens.chunks.form.fields.content.expectValue(recovered.content);
            expect(patches.requests).toHaveLength(1);
            const stored = await page.request.get(`${network.origin}${chunk.path}`);
            expect(stored.status()).toBe(200);
            expect((await stored.json()).chunk).toMatchObject(original);
        });
        // When the failure is removed and saving is retried.
        await screens.chunks.saveAndOpen();
        // Then the successful edit survives a reload.
        await screens.chunks.reloadAndExpect(recovered);
    });

    test("cancelled deletion preserves the chunk; confirmed deletion persists", async ({ page, screens, network }) => {
        // Given an authenticated user and a persisted chunk.
        const chunk = screens.chunks.current();
        const endpoint = { method: "DELETE", path: chunk.path } as const;
        const deletes = network.record(endpoint);
        // When deletion is requested and cancelled.
        const confirmation = await screens.chunks.requestDelete();
        await confirmation.close("Cancel");
        // Then no deletion request is sent.
        expect(deletes.requests).toHaveLength(0);
        await screens.chunks.reloadAndExpect(original);
        // When deletion is requested again and confirmed.
        await screens.chunks.requestDelete();
        await network.perform({ ...endpoint, status: 200 }, () => confirmation.button("Delete").click());
        // Then the chunk is deleted from both the UI and the API.
        await expect(page).toHaveURL(/\/dashboard$/);
        await page.reload();
        expect(deletes.requests).toHaveLength(1);
        const missing = await page.request.get(`${network.origin}${chunk.path}`);
        expect(missing.status()).toBe(404);
        await expect(page.getByText(original.title, { exact: true })).toHaveCount(0);
    });
});
