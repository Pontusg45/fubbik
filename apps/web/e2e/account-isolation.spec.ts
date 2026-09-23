import { AuthScreen } from "./support/screens/auth";
import { expect, test, testAccount } from "./support/test";
import { FubbikUI } from "./support/ui";

test("a second account cannot list, read or change the first account's chunk", async ({ browser, page, screens, network, baseURL }) => {
    // Given user A has a persisted chunk and user B has a separate browser session.
    await screens.auth.signUp(testAccount({ name: "Owner" }));
    const draft = { title: "Owner-only knowledge", content: "Private content stays with the owner." };
    const chunk = await screens.chunks.createChunk(draft);
    const otherContext = await browser.newContext({ baseURL: baseURL ?? "http://localhost:3001" });
    try {
        const otherPage = await otherContext.newPage();
        const otherAuth = new AuthScreen(otherPage, new FubbikUI(otherPage), network.origin);
        await otherAuth.signUp(testAccount({ name: "Other user" }));
        // When B browses chunks and requests A's chunk directly.
        await otherPage.goto("/chunks");
        await expect(otherPage.getByText(draft.title, { exact: true })).toHaveCount(0);
        const read = await otherPage.request.get(`${network.origin}${chunk.path}`);
        const update = await otherPage.request.patch(`${network.origin}${chunk.path}`, { data: { title: "Hijacked" } });
        // Then both direct operations are denied and the owner's data remains intact.
        expect(read.status()).toBe(404);
        expect(update.status()).toBe(404);
        await otherPage.goto(chunk.url);
        await expect(otherPage.getByRole("heading", { level: 1, name: draft.title })).toHaveCount(0);
        await page.goto(chunk.url);
        await screens.chunks.expectDetails(draft);
    } finally {
        await otherContext.close();
    }
});
