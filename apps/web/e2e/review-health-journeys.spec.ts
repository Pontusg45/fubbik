import type { Page } from "@playwright/test";

import { apiJson, given, openSite, seedChunk, seedConnection, then, when } from "./support/site-scenarios";
import { siteTest as test } from "./support/site-test";
import { expect } from "./support/test";

async function proposal(page: Page, origin: string, suffix: string) {
    const site = { request: page.request, origin };
    const chunk = await seedChunk(site, `Review source ${suffix}`);
    const proposed = await apiJson<{ id: string }>(site, "post", `/api/chunks/${chunk.id}/proposals`, {
        changes: { title: `Reviewed ${suffix}` },
        reason: `Review ${suffix}`
    });
    return { chunk, proposed };
}

function healthCard(page: Page, title: string) {
    return page.locator('[data-slot="card"]').filter({ has: page.getByRole("heading", { name: title }) });
}

test("approving a proposal updates the chunk and review queue", async ({ page, network }) => {
    // Given a pending change proposal.
    const suffix = crypto.randomUUID().slice(0, 8);
    const item = await given(page, "a pending proposal", () => proposal(page, network.origin, suffix));
    await openSite(page, "/review");
    const approve = page.getByRole("button", { name: "Approve", exact: true });
    // When the proposal is approved in the browser.
    await when(approve, "the proposal is approved", () =>
        network.perform({ method: "POST", path: `/api/proposals/${item.proposed.id}/approve`, status: 200 }, () => approve.click())
    );
    // Then the approved filter and chunk detail show the decision.
    await then(page, "the approved change persists", async () => {
        await page.getByRole("button", { name: "Approved", exact: true }).click();
        await expect(page.getByText("approved", { exact: true })).toBeVisible();
        await openSite(page, `/chunks/${item.chunk.id}`);
        await expect(page.getByRole("heading", { name: `Reviewed ${suffix}` })).toBeVisible();
    });
});

test("rejecting a proposal leaves the chunk unchanged", async ({ page, network }) => {
    // Given a pending proposal.
    const suffix = crypto.randomUUID().slice(0, 8);
    const item = await given(page, "a pending proposal", () => proposal(page, network.origin, suffix));
    await openSite(page, "/review");
    const reject = page.getByRole("button", { name: "Reject", exact: true });
    // When the proposal is rejected.
    await when(reject, "the proposal is rejected", () =>
        network.perform({ method: "POST", path: `/api/proposals/${item.proposed.id}/reject`, status: 200 }, () => reject.click())
    );
    // Then the rejected filter records the decision and the title is unchanged.
    await then(page, "the original chunk remains", async () => {
        await page.getByRole("button", { name: "Rejected", exact: true }).click();
        await expect(page.getByText("rejected", { exact: true })).toBeVisible();
        await openSite(page, `/chunks/${item.chunk.id}`);
        await expect(page.getByRole("heading", { name: `Review source ${suffix}` })).toBeVisible();
    });
});

test("bulk approval clears all visible pending proposals", async ({ page, network, site }) => {
    // Given two pending proposals.
    await given(page, "two pending proposals", async () => {
        await proposal(page, network.origin, `one-${crypto.randomUUID().slice(0, 5)}`);
        await proposal(page, network.origin, `two-${crypto.randomUUID().slice(0, 5)}`);
    });
    await openSite(page, "/review");
    const approveAll = page.getByRole("button", { name: "Approve all" });
    // When bulk approval is confirmed.
    await when(approveAll, "all proposals are approved", async () => {
        page.once("dialog", dialog => dialog.accept());
        await network.perform({ method: "POST", path: "/api/proposals/bulk", status: 200 }, () => approveAll.click());
    });
    // Then no pending proposals remain after reload.
    await then(page, "the queue is empty", async () => {
        await page.reload();
        await expect(page.getByText("No proposals waiting for review")).toBeVisible();
        const count = await apiJson<{ pending: number }>(site, "get", "/api/proposals/count");
        expect(count.pending).toBe(0);
    });
});

test("an unconnected chunk appears in knowledge health", async ({ page, site }) => {
    // Given a new unconnected chunk.
    const title = `Orphan ${crypto.randomUUID().slice(0, 8)}`;
    await given(page, "an unconnected chunk", () => seedChunk(site, title));
    // When knowledge health is opened.
    await when(page, "knowledge health loads", () => openSite(page, "/knowledge-health"));
    // Then the orphan section links to that chunk.
    await then(page, "the orphan is listed", async () => {
        await expect(healthCard(page, "Orphan Chunks").getByRole("link", { name: title })).toBeVisible();
    });
});

test("connecting an orphan removes it from the orphan list", async ({ page, site }) => {
    // Given two orphan chunks.
    const title = `Linked ${crypto.randomUUID().slice(0, 8)}`;
    const [source, target] = await given(page, "two orphan chunks", async () => [
        await seedChunk(site, title),
        await seedChunk(site, `Neighbor ${title}`)
    ]);
    await openSite(page, "/knowledge-health");
    await expect(healthCard(page, "Orphan Chunks").getByRole("link", { name: title, exact: true })).toBeVisible();
    // When a connection is saved and health reloaded.
    await when(page, "the chunks are connected", async () => {
        await seedConnection(site, source.id, target.id);
        await page.reload();
    });
    // Then the source no longer appears as an orphan.
    await then(page, "the orphan finding is cleared", async () => {
        await expect(healthCard(page, "Orphan Chunks").getByRole("link", { name: title, exact: true })).toHaveCount(0);
    });
});

test("thin content is listed and disappears after expansion", async ({ page, site }) => {
    // Given a chunk with almost no content.
    const title = `Thin ${crypto.randomUUID().slice(0, 8)}`;
    const chunk = await given(page, "a thin chunk", () => seedChunk(site, title, { content: "x" }));
    await openSite(page, "/knowledge-health");
    await expect(healthCard(page, "Thin Chunks").getByRole("link", { name: title })).toBeVisible();
    // When the content is expanded and health reloaded.
    await when(page, "the chunk content is expanded", async () => {
        await apiJson(site, "patch", `/api/chunks/${chunk.id}`, { content: "A complete explanation. ".repeat(50) });
        await page.reload();
    });
    // Then it is no longer classified as thin.
    await then(page, "the thin finding is cleared", async () => {
        await expect(healthCard(page, "Thin Chunks").getByRole("link", { name: title })).toHaveCount(0);
    });
});
