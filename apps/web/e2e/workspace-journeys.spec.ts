import type { Page } from "@playwright/test";

import { AuthScreen } from "./support/screens/auth";
import {
    apiJson,
    given,
    openSite,
    seedSpace,
    seedWorkspace,
    seedWorkspaceWithSpace,
    then,
    when,
    workspaceSpaces
} from "./support/site-scenarios";
import { siteTest as test } from "./support/site-test";
import { expect, testAccount } from "./support/test";
import { FubbikUI } from "./support/ui";

function row(page: Page, name: string) {
    return page.getByText(name, { exact: true }).locator("xpath=ancestor::div[contains(@class,'py-3')][1]");
}

test("a workspace created in the page survives navigation and reload", async ({ page, network }) => {
    // Given the workspace creation form.
    await given(page, "the workspace page", () => openSite(page, "/workspaces"));
    const name = `Workspace ${crypto.randomUUID().slice(0, 8)}`;
    const input = page.getByPlaceholder("Name", { exact: true });
    // When a named workspace is created.
    await when(input, "a workspace is created", async () => {
        await input.fill(name);
        await page.getByPlaceholder("Description (optional)").fill("Shared research space");
        await network.perform({ method: "POST", path: "/api/workspaces", status: 201 }, () =>
            page.getByRole("button", { name: "Create", exact: true }).click()
        );
    });
    // Then it appears after leaving, going Back, and reloading.
    await then(page, "the workspace remains available", async () => {
        await expect(page.getByText(name, { exact: true })).toBeVisible();
        await openSite(page, "/spaces");
        await page.goBack();
        await page.reload();
        await expect(page.getByText(name, { exact: true })).toBeVisible();
    });
});

test("a space can be assigned to a workspace", async ({ page, site }) => {
    // Given a workspace and a separate space.
    const name = `Workspace ${crypto.randomUUID().slice(0, 8)}`;
    const spaceName = `Member ${crypto.randomUUID().slice(0, 8)}`;
    const [workspace, space] = await given(
        page,
        "a workspace and space",
        async () => [await seedWorkspace(site, name), await seedSpace(site, spaceName)] as const
    );
    await openSite(page, "/workspaces");
    await row(page, name)
        .getByRole("button", { name: new RegExp(name) })
        .click();
    const add = row(page, name).getByRole("button", { name: "Add Space" });
    // When the space is selected from the workspace popover.
    await when(add, "the space is assigned", async () => {
        await add.click();
        await page.getByRole("dialog").getByRole("button", { name: spaceName }).click();
        await expect
            .poll(async () => {
                return (await workspaceSpaces(site, workspace.id)).some(item => item.id === space.id);
            })
            .toBe(true);
    });
    // Then the membership persists after reload.
    await then(page, "the workspace includes the space", async () => {
        await page.reload();
        await row(page, name)
            .getByRole("button", { name: new RegExp(name) })
            .click();
        await expect(row(page, name).getByText(spaceName, { exact: true })).toBeVisible();
        expect((await workspaceSpaces(site, workspace.id)).map(item => item.id)).toContain(space.id);
    });
});

test("removing a space from a workspace leaves the space itself", async ({ page, network, site }) => {
    // Given a workspace with one assigned space.
    const name = `Workspace ${crypto.randomUUID().slice(0, 8)}`;
    const spaceName = `Removable ${crypto.randomUUID().slice(0, 8)}`;
    const { workspace, space } = await given(page, "an assigned space", () => seedWorkspaceWithSpace(site, name, spaceName));
    await openSite(page, "/workspaces");
    await row(page, name)
        .getByRole("button", { name: new RegExp(name) })
        .click();
    const remove = row(page, name).getByText(spaceName, { exact: true }).locator("xpath=../..").locator("button");
    // When the membership is removed.
    await when(remove, "the space is removed from the workspace", () =>
        network.perform({ method: "DELETE", path: `/api/workspaces/${workspace.id}/spaces/${space.id}`, status: 200 }, () => remove.click())
    );
    // Then the workspace no longer contains it, while the space still exists.
    await then(page, "the space itself remains", async () => {
        expect((await workspaceSpaces(site, workspace.id)).map(item => item.id)).not.toContain(space.id);
        const spaces = await apiJson<Array<{ id: string }>>(site, "get", "/api/spaces");
        expect(spaces.map(item => item.id)).toContain(space.id);
    });
});

test("deleting a workspace does not delete its member space", async ({ page, network, site }) => {
    // Given a workspace with an assigned space.
    const name = `Delete workspace ${crypto.randomUUID().slice(0, 8)}`;
    const { workspace, space } = await given(page, "a populated workspace", () =>
        seedWorkspaceWithSpace(site, name, `Keep space ${crypto.randomUUID().slice(0, 8)}`)
    );
    await openSite(page, "/workspaces");
    const deleteButton = row(page, name).locator("button").last();
    // When workspace deletion is confirmed.
    await when(deleteButton, "the workspace is deleted", async () => {
        await deleteButton.click();
        await network.perform({ method: "DELETE", path: `/api/workspaces/${workspace.id}`, status: 200 }, () =>
            page.getByRole("dialog", { name: "Delete workspace" }).getByRole("button", { name: "Delete" }).click()
        );
    });
    // Then the workspace is gone and its space remains in the API.
    await then(page, "only the workspace is removed", async () => {
        await expect(page.getByText(name, { exact: true })).toHaveCount(0);
        const spaces = await apiJson<Array<{ id: string }>>(site, "get", "/api/spaces");
        expect(spaces.map(item => item.id)).toContain(space.id);
    });
});

test("another account cannot see a private workspace", async ({ browser, page, network, baseURL, site }) => {
    // Given a workspace owned by account A.
    const name = `Private workspace ${crypto.randomUUID().slice(0, 8)}`;
    const owned = await given(page, "an account-owned workspace", () => seedWorkspace(site, name));
    const otherContext = await browser.newContext({ baseURL: baseURL ?? "http://localhost:3001" });
    try {
        const otherPage = await otherContext.newPage();
        const auth = new AuthScreen(otherPage, new FubbikUI(otherPage), network.origin);
        // When account B signs in and opens Workspaces.
        await when(otherPage, "another account opens workspaces", async () => {
            await auth.signUp(testAccount({ name: "Other workspace user" }));
            await openSite(otherPage, "/workspaces");
        });
        // Then account A's workspace is absent from both the page and API.
        await then(otherPage, "the private workspace is hidden", async () => {
            await expect(otherPage.getByText(name, { exact: true })).toHaveCount(0);
            const listing = await apiJson<Array<{ id: string }>>(
                { request: otherPage.request, origin: network.origin },
                "get",
                "/api/workspaces"
            );
            expect(listing.map(item => item.id)).not.toContain(owned.id);
            expect((await otherPage.request.get(`${network.origin}/api/workspaces/${owned.id}`)).status()).toBe(404);
        });
    } finally {
        await otherContext.close();
    }
});
