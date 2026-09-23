import {
    apiJson,
    given,
    openSite,
    seedChunk,
    seedCoveredChunk,
    seedRequirement,
    setRequirementChunks,
    then,
    when
} from "./support/site-scenarios";
import { siteTest as test } from "./support/site-test";
import { expect } from "./support/test";

async function learningPath(page: Parameters<typeof openSite>[0], origin: string) {
    const site = { request: page.request, origin };
    const first = await seedChunk(site, `Learn first ${crypto.randomUUID().slice(0, 6)}`);
    const second = await seedChunk(site, `Learn second ${crypto.randomUUID().slice(0, 6)}`);
    const title = `Reading path ${crypto.randomUUID().slice(0, 6)}`;
    const path = await apiJson<{ id: string }>(site, "post", "/api/learning-paths", {
        title,
        description: "Read in sequence",
        chunkIds: [first.id, second.id]
    });
    return { path, first, second, title };
}

async function requirement(page: Parameters<typeof openSite>[0], origin: string) {
    const title = `Coverage requirement ${crypto.randomUUID().slice(0, 6)}`;
    return seedRequirement({ request: page.request, origin }, title);
}

test("learning path list opens its ordered chunk detail", async ({ page, network }) => {
    // Given an API-created reading path with two chunks.
    const seeded = await given(page, "an ordered learning path", () => learningPath(page, network.origin));
    await openSite(page, "/learn");
    const link = page.getByRole("link", { name: new RegExp(seeded.title) });
    // When the path is opened from the list.
    await when(link, "the reading path opens", () => link.click());
    // Then both chunks appear in source order and are navigable.
    await then(page, "the reading order is preserved", async () => {
        await expect(page).toHaveURL(new RegExp(`/learn/${seeded.path.id}$`));
        const entries = page.locator("main ol li a");
        await expect(entries).toHaveCount(2);
        await expect(entries.first()).toContainText("Learn first");
        await expect(entries.last()).toContainText("Learn second");
    });
});

test("learning path detail reflects a persisted order change", async ({ page, network, site }) => {
    // Given a saved path with two chunks.
    const seeded = await given(page, "a two-chunk learning path", () => learningPath(page, network.origin));
    await openSite(page, `/learn/${seeded.path.id}`);
    // When its order changes through the API and the page reloads.
    await when(page, "the path order is reversed", async () => {
        await apiJson(site, "patch", `/api/learning-paths/${seeded.path.id}`, {
            chunkIds: [seeded.second.id, seeded.first.id]
        });
        await page.reload();
    });
    // Then the browser reflects the persisted order.
    await then(page, "the second chunk now leads", async () => {
        const entries = page.locator("main ol li a");
        await expect(entries.first()).toContainText("Learn second");
        await expect(entries.last()).toContainText("Learn first");
    });
});

test("coverage lists a new chunk as uncovered", async ({ page, site }) => {
    // Given a chunk with no requirement link.
    const title = `Uncovered ${crypto.randomUUID().slice(0, 7)}`;
    await given(page, "an unlinked chunk", () => seedChunk(site, title));
    // When coverage is opened.
    await when(page, "coverage loads", () => openSite(page, "/coverage"));
    // Then the chunk appears under Uncovered.
    await then(page, "the chunk is uncovered", async () => {
        await expect(page.getByText("Uncovered (1)")).toBeVisible();
        await expect(page.getByRole("link", { name: title })).toBeVisible();
    });
});

test("linking a requirement moves a chunk into covered", async ({ page, network, site }) => {
    // Given a chunk and a separate requirement.
    const title = `Covered ${crypto.randomUUID().slice(0, 7)}`;
    const [chunk, req] = await given(
        page,
        "a chunk and requirement",
        async () => [await seedChunk(site, title), await requirement(page, network.origin)] as const
    );
    // When the requirement is linked and coverage reloads.
    await when(page, "the requirement is linked", async () => {
        await setRequirementChunks(site, req.id, [chunk.id]);
        await openSite(page, "/coverage");
    });
    // Then the covered list and API both include that chunk.
    await then(page, "coverage records the link", async () => {
        await expect(page.getByText("Covered (1)")).toBeVisible();
        await page.getByRole("button", { name: "Show", exact: true }).click();
        await expect(page.getByRole("link", { name: title })).toBeVisible();
        const data = await apiJson<{ covered: Array<{ id: string }> }>(site, "get", "/api/requirements/coverage");
        expect(data.covered.map(item => item.id)).toContain(chunk.id);
    });
});

test("coverage matrix removes a link after requirement unlink", async ({ page, site }) => {
    // Given a covered chunk and visible coverage matrix.
    const title = `Matrix coverage ${crypto.randomUUID().slice(0, 6)}`;
    const { requirement: req } = await given(page, "a covered chunk", () => seedCoveredChunk(site, title));
    await openSite(page, "/coverage");
    await page.getByRole("button", { name: "Show Matrix" }).click();
    await expect(page.getByRole("table")).toContainText(title);
    // When the link is removed and coverage reloaded.
    await when(page, "the coverage link is removed", async () => {
        await setRequirementChunks(site, req.id, []);
        await page.reload();
    });
    // Then the chunk returns to Uncovered.
    await then(page, "the chunk is uncovered again", async () => {
        await expect(page.getByText("Uncovered (1)")).toBeVisible();
        await expect(page.getByRole("link", { name: title })).toBeVisible();
    });
});
