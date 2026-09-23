import type { Page } from "@playwright/test";

import { siteTest as test } from "./support/site-test";
import { expect } from "./support/test";

async function createFeature(page: Page, origin: string, name: string, fields: Record<string, unknown> = {}) {
    const response = await page.request.post(`${origin}/api/features`, { data: { name, ...fields } });
    expect(response.status()).toBe(201);
    return (await response.json()) as { id: string; name: string };
}

function featureCard(page: Page, name: string) {
    return page.getByText(name, { exact: true }).locator("xpath=ancestor::div[contains(@class,'group')][1]");
}

async function openFeatures(page: Page) {
    await page.goto("/features");
    await page.waitForLoadState("networkidle");
}

test("a feature cannot be created without a name", async ({ page, network }) => {
    // Given an empty New Feature dialog.
    await openFeatures(page);
    await page.getByRole("button", { name: "New Feature" }).click();
    const writes = network.record({ method: "POST", path: "/api/features" });
    // When the name remains blank.
    // Then Create is disabled and no request is sent.
    await expect(page.getByRole("dialog", { name: "New Feature" }).getByRole("button", { name: "Create" })).toBeDisabled();
    expect(writes.requests).toHaveLength(0);
});

test("feature description and color persist from the dialog", async ({ page, network }) => {
    // Given a new feature dialog with metadata.
    await openFeatures(page);
    const name = `metadata-${crypto.randomUUID().slice(0, 6)}`;
    await page.getByRole("button", { name: "New Feature" }).click();
    const dialog = page.getByRole("dialog", { name: "New Feature" });
    await dialog.getByLabel("Name").fill(name);
    await dialog.getByLabel(/Description/).fill("A feature description.");
    await dialog.getByLabel("Color").fill("#123456");
    // When the feature is created.
    await network.perform({ method: "POST", path: "/api/features", status: 201 }, () =>
        dialog.getByRole("button", { name: "Create" }).click()
    );
    // Then the stored metadata matches the inputs.
    const list = (await (await page.request.get(`${network.origin}/api/features`)).json()) as Array<Record<string, unknown>>;
    expect(list.find(item => item.name === name)).toMatchObject({ description: "A feature description.", color: "#123456" });
});

test("two features can be active simultaneously", async ({ page, network }) => {
    // Given two inactive features.
    const first = await createFeature(page, network.origin, "First active feature");
    const second = await createFeature(page, network.origin, "Second active feature");
    await openFeatures(page);
    // When both Activate controls are clicked.
    await featureCard(page, first.name).getByRole("button", { name: "Activate", exact: true }).click();
    await expect(featureCard(page, first.name).getByRole("button", { name: "Deactivate" })).toBeVisible();
    await featureCard(page, second.name).getByRole("button", { name: "Activate", exact: true }).click();
    // Then both IDs are in the active API selection.
    await expect
        .poll(async () => (await (await page.request.get(`${network.origin}/api/features/active`)).json()) as string[])
        .toEqual(expect.arrayContaining([first.id, second.id]));
});

test("deactivating one feature leaves another selected", async ({ page, network }) => {
    // Given two active features.
    const first = await createFeature(page, network.origin, "First selected feature", { priority: 1 });
    const second = await createFeature(page, network.origin, "Second selected feature", { priority: 2 });
    await page.request.patch(`${network.origin}/api/features/${first.id}`, { data: { status: "active" } });
    await page.request.patch(`${network.origin}/api/features/${second.id}`, { data: { status: "active" } });
    await page.request.put(`${network.origin}/api/features/active`, { data: { featureIds: [first.id, second.id] } });
    await openFeatures(page);
    // When only the first is deactivated.
    await featureCard(page, first.name).getByRole("button", { name: "Deactivate" }).click();
    // Then the second stays selected while the first leaves the selection.
    await expect
        .poll(async () => (await (await page.request.get(`${network.origin}/api/features/active`)).json()) as string[])
        .toEqual([second.id]);
});

test("active feature selection survives navigation", async ({ page, network }) => {
    // Given an active feature selected in the UI.
    const feature = await createFeature(page, network.origin, "Navigating feature");
    await openFeatures(page);
    await featureCard(page, feature.name).getByRole("button", { name: "Activate" }).click();
    await expect(featureCard(page, feature.name).getByRole("button", { name: "Deactivate" })).toBeVisible();
    // When the user navigates away and returns.
    await page.goto("/dashboard");
    await openFeatures(page);
    // Then the feature remains active.
    await expect(featureCard(page, feature.name).getByRole("button", { name: "Deactivate" })).toBeVisible();
    expect((await (await page.request.get(`${network.origin}/api/features/active`)).json()) as string[]).toContain(feature.id);
});

test("canceling feature archive leaves it inactive", async ({ page, network }) => {
    // Given an inactive feature.
    const feature = await createFeature(page, network.origin, "Archive cancel feature");
    await openFeatures(page);
    // When Archive is requested and canceled.
    await page.getByRole("button", { name: `Actions for ${feature.name}` }).click();
    await page.getByRole("menuitem", { name: "Archive" }).click();
    await page.getByRole("dialog", { name: "Archive feature" }).getByRole("button", { name: "Cancel" }).click();
    // Then the feature remains inactive.
    const stored = await page.request.get(`${network.origin}/api/features/${feature.id}`);
    expect((await stored.json()).feature.status).toBe("inactive");
});

test("canceling feature deletion keeps it in the list", async ({ page, network }) => {
    // Given an inactive feature.
    const feature = await createFeature(page, network.origin, "Delete cancel feature");
    await openFeatures(page);
    const writes = network.record({ method: "DELETE", path: `/api/features/${feature.id}` });
    // When Delete is requested and canceled.
    await page.getByRole("button", { name: `Actions for ${feature.name}` }).click();
    await page.getByRole("menuitem", { name: "Delete" }).click();
    await page.getByRole("dialog", { name: "Delete feature" }).getByRole("button", { name: "Cancel" }).click();
    // Then no delete request occurs and the feature remains visible.
    expect(writes.requests).toHaveLength(0);
    await expect(featureCard(page, feature.name)).toBeVisible();
});

test("confirmed feature deletion removes its API record", async ({ page, network }) => {
    // Given an inactive feature.
    const feature = await createFeature(page, network.origin, "Delete confirm feature");
    await openFeatures(page);
    // When Delete is confirmed.
    await page.getByRole("button", { name: `Actions for ${feature.name}` }).click();
    await page.getByRole("menuitem", { name: "Delete" }).click();
    await network.perform({ method: "DELETE", path: `/api/features/${feature.id}`, status: 200 }, () =>
        page.getByRole("dialog", { name: "Delete feature" }).getByRole("button", { name: "Delete" }).click()
    );
    // Then the feature is absent after reload.
    await page.reload();
    await expect(page.getByText(feature.name, { exact: true })).toHaveCount(0);
    expect((await page.request.get(`${network.origin}/api/features/${feature.id}`)).status()).toBe(404);
});

test("deleting the last feature displays the empty state", async ({ page, network }) => {
    // Given a single feature.
    const feature = await createFeature(page, network.origin, "Last feature");
    await openFeatures(page);
    // When it is deleted.
    await page.getByRole("button", { name: `Actions for ${feature.name}` }).click();
    await page.getByRole("menuitem", { name: "Delete" }).click();
    await network.perform({ method: "DELETE", path: `/api/features/${feature.id}`, status: 200 }, () =>
        page.getByRole("dialog", { name: "Delete feature" }).getByRole("button", { name: "Delete" }).click()
    );
    // Then the empty state appears.
    await expect(page.getByText("No features yet", { exact: true })).toBeVisible();
});

test("a feature shows its chunk delta count", async ({ page, network }) => {
    // Given a feature and chunk with an overlay delta.
    const feature = await createFeature(page, network.origin, "Delta count feature");
    const created = await page.request.post(`${network.origin}/api/chunks`, { data: { title: "Delta count chunk", content: "Base" } });
    const chunk = (await created.json()) as { id: string };
    const delta = await page.request.put(`${network.origin}/api/chunks/${chunk.id}/deltas/${feature.id}`, {
        data: { delta: { content: "Changed by feature" } }
    });
    expect(delta.status()).toBe(200);
    // When the features page opens.
    await openFeatures(page);
    // Then the card reports one chunk delta.
    await expect(featureCard(page, feature.name).getByText("1 delta", { exact: true })).toBeVisible();
});
