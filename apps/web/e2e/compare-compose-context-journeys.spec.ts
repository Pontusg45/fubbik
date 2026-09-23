import { readFile } from "node:fs/promises";

import { apiJson, given, openSite, seedChunk, then, when } from "./support/site-scenarios";
import { siteTest as test } from "./support/site-test";
import { expect } from "./support/test";

test("compare selects two chunks and shows their content", async ({ page, site }) => {
    // Given two chunks with different content.
    const suffix = crypto.randomUUID().slice(0, 8);
    const leftTitle = `Left ${suffix}`;
    const rightTitle = `Right ${suffix}`;
    await given(page, "two distinct chunks", async () => {
        await seedChunk(site, leftTitle, { content: "Left-only insight" });
        await seedChunk(site, rightTitle, { content: "Right-only insight" });
    });
    await openSite(page, "/compare");
    const searches = page.getByPlaceholder("Search chunks...");
    // When each side is selected by title.
    await when(searches.first(), "both comparison sides are selected", async () => {
        await searches.first().fill(leftTitle);
        await page.getByRole("button", { name: leftTitle }).click();
        await searches.last().fill(rightTitle);
        await page.getByRole("button", { name: rightTitle }).click();
    });
    // Then both source contents appear.
    await then(page, "both chunks are compared", async () => {
        await expect(page.getByText("Left-only insight", { exact: true })).toBeVisible();
        await expect(page.getByText("Right-only insight", { exact: true })).toBeVisible();
    });
});

test("a comparison URL restores the diff for two saved chunks", async ({ page, site }) => {
    // Given two saved chunks and a direct comparison URL.
    const [left, right] = await given(page, "two chunks with a comparison URL", async () => [
        await seedChunk(site, "Compare old", { content: "Unchanged\nBefore" }),
        await seedChunk(site, "Compare new", { content: "Unchanged\nAfter" })
    ]);
    // When the URL is opened and diff mode selected.
    await when(page, "the comparison URL opens", async () => {
        await openSite(page, `/compare?left=${left.id}&right=${right.id}`);
        await page.getByRole("button", { name: "Show diff" }).click();
    });
    // Then both changed lines are visible after reload.
    await then(page, "the compared chunks remain selected", async () => {
        await expect(page.getByText("Before", { exact: true })).toBeVisible();
        await expect(page.getByText("After", { exact: true })).toBeVisible();
        await page.reload();
        await expect(page.getByText("Selected: Compare old")).toBeVisible();
        await expect(page.getByText("Selected: Compare new")).toBeVisible();
    });
});

test("compose displays the content returned by a query", async ({ page, site }) => {
    // Given two note chunks with distinct content.
    const suffix = crypto.randomUUID().slice(0, 8);
    await given(page, "two note chunks", async () => {
        await seedChunk(site, `Alpha ${suffix}`, { content: "Alpha composition body" });
        await seedChunk(site, `Beta ${suffix}`, { content: "Beta composition body" });
    });
    // When compose loads a type query.
    await when(page, "the note composition opens", () => openSite(page, "/compose?q=type%3Anote"));
    // Then both source bodies are present and sorting is reflected in the URL.
    await then(page, "the composition reflects both sources", async () => {
        await expect(page.getByText("Alpha composition body", { exact: true })).toBeVisible();
        await expect(page.getByText("Beta composition body", { exact: true })).toBeVisible();
        await page.getByLabel("Sort:").selectOption("title");
        await expect(page).toHaveURL(/sort=title/);
    });
});

test("compose downloads Markdown containing the source text", async ({ page, site }) => {
    // Given a note whose text can be identified in an export.
    const title = `Export ${crypto.randomUUID().slice(0, 8)}`;
    await given(page, "an exportable note", () => seedChunk(site, title, { content: "Exported body proof" }));
    await openSite(page, "/compose?q=type%3Anote");
    await expect(page.getByRole("heading", { name: title })).toBeVisible();
    const downloadButton = page.getByRole("button", { name: "Download" });
    // When the composed Markdown is downloaded.
    const download = await when(downloadButton, "the composition is downloaded", async () => {
        const pending = page.waitForEvent("download");
        await downloadButton.click();
        return pending;
    });
    // Then the file contains the source title and body.
    await then(page, "the exported Markdown matches the source", async () => {
        const content = await readFile(await download.path(), "utf8");
        expect(content).toContain(`## ${title}`);
        expect(content).toContain("Exported body proof");
    });
});

test("context search resolves a chunk by its file reference", async ({ page, site }) => {
    // Given a chunk that documents a specific file.
    const title = `File context ${crypto.randomUUID().slice(0, 8)}`;
    const filePath = `src/context-${crypto.randomUUID().slice(0, 8)}.ts`;
    const chunk = await given(page, "a file-referenced chunk", async () => {
        const created = await seedChunk(site, title);
        await apiJson(site, "put", `/api/chunks/${created.id}/file-refs`, [{ path: filePath, relation: "documents" }]);
        await expect
            .poll(async () => {
                const result = await apiJson<{ chunks: Array<{ id: string }> }>(
                    site,
                    "get",
                    `/api/context/for-file?path=${encodeURIComponent(filePath)}&format=json-legacy`
                );
                return result.chunks.some(item => item.id === created.id);
            })
            .toBe(true);
        return created;
    });
    await openSite(page, "/context");
    const input = page.getByPlaceholder(/Enter a file path/);
    // When the file path is searched.
    await when(input, "the referenced file is searched", async () => {
        await input.fill(filePath);
        await input.press("Enter");
    });
    // Then context links to the matching chunk.
    await then(page, "the matching chunk is available", async () => {
        const link = page.getByRole("link", { name: title });
        await expect(link).toBeVisible();
        await link.click();
        await expect(page).toHaveURL(new RegExp(`/chunks/${chunk.id}$`));
    });
});

test("context search requires a file path before requesting results", async ({ page, network }) => {
    // Given an untouched context search.
    await given(page, "an empty context search", () => openSite(page, "/context"));
    const search = page.getByRole("button", { name: "Search", exact: true });
    // When the path contains only whitespace.
    await when(page.getByPlaceholder(/Enter a file path/), "only whitespace is entered", () =>
        page.getByPlaceholder(/Enter a file path/).fill("   ")
    );
    // Then Search stays unavailable and no context request is made.
    await then(page, "the empty search is blocked", async () => {
        const calls = network.record({ method: "GET", path: "/api/context/for-file" });
        await expect(search).toBeDisabled();
        expect(calls.requests).toHaveLength(0);
        calls.stop();
    });
});
