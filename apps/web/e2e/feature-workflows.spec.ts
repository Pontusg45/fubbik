import { expect, test, testAccount } from "./support/test";

test("feature activation, deactivation and archive persist", async ({ page, screens, network }) => {
    // Given a newly created feature.
    await screens.auth.signUp(testAccount());
    await page.goto("/features");
    await page.waitForLoadState("networkidle");
    const name = `e2e-feature-${crypto.randomUUID().slice(0, 8)}`;
    await screens.feature.createFeature(name);
    const card = page.getByText(name, { exact: true }).locator("xpath=ancestor::div[contains(@class,'group')][1]");
    const list = await page.request.get(`${network.origin}/api/features`);
    const feature = ((await list.json()) as Array<{ id: string; name: string }>).find(item => item.name === name);
    expect(feature?.id).toBeTruthy();
    const id = feature!.id;
    // When the user activates it, then the active selection survives reload.
    await network.perform({ method: "PUT", path: "/api/features/active", status: 200 }, () =>
        card.getByRole("button", { name: "Activate", exact: true }).click()
    );
    await page.reload();
    await expect(card.getByText("active", { exact: true })).toHaveCount(2);
    // When it is deactivated, then both the selection and stored status change.
    await network.perform({ method: "PATCH", path: `/api/features/${id}`, status: 200 }, () =>
        card.getByRole("button", { name: "Deactivate", exact: true }).click()
    );
    await page.reload();
    await expect(card.getByText("inactive", { exact: true })).toBeVisible();
    await expect(card.getByText("active", { exact: true })).toHaveCount(0);
    // When the feature is archived, then its status persists after reload.
    await page.getByRole("button", { name: `Actions for ${name}` }).click();
    await page.getByRole("menuitem", { name: "Archive" }).click();
    await network.perform({ method: "PATCH", path: `/api/features/${id}`, status: 200 }, () =>
        page.getByRole("dialog", { name: "Archive feature" }).getByRole("button", { name: "Archive" }).click()
    );
    await page.reload();
    await expect(card.getByText("archived", { exact: true })).toBeVisible();
});
