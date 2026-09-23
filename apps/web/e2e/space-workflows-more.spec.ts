import type { Page } from "@playwright/test";

import { siteTest as test } from "./support/site-test";
import { expect } from "./support/test";

function spaceRow(page: Page, name: string) {
    return page.getByRole("paragraph").filter({ hasText: name }).locator("xpath=../..");
}

test("a blank space name cannot be submitted", async ({ page, network }) => {
    // Given the space creation form.
    await page.goto("/spaces");
    const writes = network.record({ method: "POST", path: "/api/spaces" });
    // When the name contains only whitespace.
    await page.getByPlaceholder("Name", { exact: true }).fill("   ");
    // Then Add is disabled and no write occurs.
    await expect(page.getByRole("button", { name: "Add", exact: true })).toBeDisabled();
    expect(writes.requests).toHaveLength(0);
});

test("two spaces appear in the list after creation", async ({ page, screens, network }) => {
    // Given an account with no spaces.
    const first = `First space ${crypto.randomUUID().slice(0, 6)}`;
    const second = `Second space ${crypto.randomUUID().slice(0, 6)}`;
    // When two spaces are created.
    await screens.spaces.create(first);
    await screens.spaces.create(second);
    // Then both are listed by the API and page.
    const listed = (await (await page.request.get(`${network.origin}/api/spaces`)).json()) as Array<{ name: string }>;
    expect(listed.map(space => space.name)).toEqual(expect.arrayContaining([first, second]));
    await expect(spaceRow(page, first)).toBeVisible();
    await expect(spaceRow(page, second)).toBeVisible();
});

test("the space switcher selects a second space", async ({ page, screens }) => {
    // Given two spaces.
    const first = `First ${crypto.randomUUID().slice(0, 6)}`;
    const second = `Second ${crypto.randomUUID().slice(0, 6)}`;
    await screens.spaces.create(first);
    await screens.spaces.create(second);
    // When the second is chosen from the switcher.
    await page.getByRole("button", { name: first }).click();
    await page.getByRole("menuitem", { name: second }).click();
    // Then the header identifies the selected space.
    await expect(page.getByRole("button", { name: second })).toBeVisible();
});

test("the selected space survives a page reload", async ({ page, screens }) => {
    // Given two spaces with the second selected.
    const first = `First ${crypto.randomUUID().slice(0, 6)}`;
    const second = `Second ${crypto.randomUUID().slice(0, 6)}`;
    await screens.spaces.create(first);
    const id = await screens.spaces.create(second);
    await page.getByRole("button", { name: first }).click();
    await page.getByRole("menuitem", { name: second }).click();
    // When the page reloads.
    await page.reload();
    // Then the selection and stored ID remain.
    await expect(page.getByRole("button", { name: second })).toBeVisible();
    expect(await page.evaluate(() => localStorage.getItem("active-space"))).toBe(id);
});

test("chunk browsing is scoped to the selected space", async ({ page, screens, network }) => {
    // Given a separate chunk in each of two spaces.
    const first = `First ${crypto.randomUUID().slice(0, 6)}`;
    const second = `Second ${crypto.randomUUID().slice(0, 6)}`;
    const firstId = await screens.spaces.create(first);
    const secondId = await screens.spaces.create(second);
    await page.request.post(`${network.origin}/api/chunks`, { data: { title: "First scoped chunk", spaceIds: [firstId] } });
    await page.request.post(`${network.origin}/api/chunks`, { data: { title: "Second scoped chunk", spaceIds: [secondId] } });
    // When the first space is selected.
    await page.getByRole("button", { name: first }).click();
    await page.getByRole("menuitem", { name: first }).click();
    await page.goto("/chunks");
    // Then its chunk appears and the other is hidden.
    await expect(page.getByText("First scoped chunk", { exact: true })).toBeVisible();
    await expect(page.getByText("Second scoped chunk", { exact: true })).toHaveCount(0);
});

test("the space dashboard lists a recent chunk", async ({ page, screens, network }) => {
    // Given a space with an associated chunk.
    const name = `Dashboard ${crypto.randomUUID().slice(0, 6)}`;
    const id = await screens.spaces.create(name);
    await page.request.post(`${network.origin}/api/chunks`, { data: { title: "Recent space chunk", spaceIds: [id] } });
    // When its dashboard opens.
    await page.goto(`/spaces/${id}`);
    // Then the space and recent chunk appear.
    await expect(page.getByRole("heading", { level: 1, name })).toBeVisible();
    await expect(page.getByRole("link", { name: /Recent space chunk/ })).toBeVisible();
});

test("canceling a space reset preserves its chunk", async ({ page, screens, network }) => {
    // Given a space containing a chunk.
    const name = `Reset cancel ${crypto.randomUUID().slice(0, 6)}`;
    const id = await screens.spaces.create(name);
    await page.request.post(`${network.origin}/api/chunks`, { data: { title: "Reset survivor", spaceIds: [id] } });
    const writes = network.record({ method: "POST", path: `/api/spaces/${id}/reset` });
    // When the reset dialog is canceled.
    await spaceRow(page, name).getByRole("button", { name: /Reset/ }).click();
    await page.getByRole("dialog", { name: "Reset space" }).getByRole("button", { name: "Cancel" }).click();
    // Then no reset is sent and the chunk remains.
    expect(writes.requests).toHaveLength(0);
    await page.goto(`/spaces/${id}`);
    await expect(page.getByText("Reset survivor", { exact: true })).toBeVisible();
});

test("confirming a space reset removes its data but keeps the space", async ({ page, screens, network }) => {
    // Given a space containing a chunk.
    const name = `Reset confirm ${crypto.randomUUID().slice(0, 6)}`;
    const id = await screens.spaces.create(name);
    await page.request.post(`${network.origin}/api/chunks`, { data: { title: "Reset target", spaceIds: [id] } });
    // When the reset is confirmed.
    await network.perform({ method: "POST", path: `/api/spaces/${id}/reset`, status: 200 }, async () => {
        await spaceRow(page, name).getByRole("button", { name: /Reset/ }).click();
        await page.getByRole("dialog", { name: "Reset space" }).getByRole("button", { name: "Reset" }).click();
    });
    // Then the space remains but its chunk disappears.
    await page.goto(`/spaces/${id}`);
    await expect(page.getByRole("heading", { name })).toBeVisible();
    await expect(page.getByText("Reset target", { exact: true })).toHaveCount(0);
});

test("canceling space deletion preserves the space", async ({ page, screens, network }) => {
    // Given a persisted space.
    const name = `Delete cancel ${crypto.randomUUID().slice(0, 6)}`;
    const id = await screens.spaces.create(name);
    const writes = network.record({ method: "DELETE", path: `/api/spaces/${id}` });
    // When deletion is canceled.
    await spaceRow(page, name)
        .getByRole("button", { name: /Delete space/ })
        .click();
    await page.getByRole("dialog", { name: "Delete space" }).getByRole("button", { name: "Cancel" }).click();
    // Then the API receives no delete and the space remains listed.
    expect(writes.requests).toHaveLength(0);
    await page.reload();
    await expect(spaceRow(page, name)).toBeVisible();
});

test("confirming space deletion removes its API record", async ({ page, screens, network }) => {
    // Given a persisted space.
    const name = `Delete confirm ${crypto.randomUUID().slice(0, 6)}`;
    const id = await screens.spaces.create(name);
    // When deletion is confirmed.
    await network.perform({ method: "DELETE", path: `/api/spaces/${id}`, status: 200 }, async () => {
        await spaceRow(page, name)
            .getByRole("button", { name: /Delete space/ })
            .click();
        await page.getByRole("dialog", { name: "Delete space" }).getByRole("button", { name: "Delete" }).click();
    });
    // Then it disappears from the list and cannot be fetched.
    await page.reload();
    await expect(spaceRow(page, name)).toHaveCount(0);
    expect((await page.request.get(`${network.origin}/api/spaces/${id}`)).status()).toBe(404);
});
