import { expect, test, testAccount } from "./support/test";

test("a plan keeps seeded tasks when duplicated and can be deleted", async ({ page, screens, network }) => {
    // Given an authenticated user creating a plan with two initial tasks.
    await screens.auth.signUp(testAccount());
    await page.goto("/plans/new");
    await page.waitForLoadState("networkidle");
    const title = `E2E plan ${crypto.randomUUID().slice(0, 8)}`;
    await page.getByLabel("Title", { exact: true }).fill(title);
    await page.getByRole("button", { name: /Seed initial tasks/ }).click();
    await page.getByPlaceholder(/One task title per line/).fill("Draft outline\nReview outline");
    // When the plan is submitted, then its detail page contains both tasks after reload.
    const created = await network.perform({ method: "POST", path: "/api/plans", status: 200 }, () =>
        page.getByRole("button", { name: "Create Plan" }).click()
    );
    const source = (await created.json()) as { id: string };
    await page.waitForURL(`**/plans/${source.id}`);
    await page.reload();
    await expect(page.getByRole("heading", { level: 1, name: title })).toBeVisible();
    await expect(page.getByText("Draft outline", { exact: true })).toBeVisible();
    await expect(page.getByText("Review outline", { exact: true })).toBeVisible();
    // When the plan is duplicated, then a distinct plan retains its tasks.
    const copied = await network.perform({ method: "POST", path: `/api/plans/${source.id}/duplicate`, status: 200 }, () =>
        page.getByRole("button", { name: "Duplicate" }).click()
    );
    const duplicate = (await copied.json()) as { id: string; title: string };
    expect(duplicate.id).not.toBe(source.id);
    await page.waitForURL(`**/plans/${duplicate.id}`);
    await page.reload();
    await expect(page.getByRole("heading", { level: 1, name: duplicate.title })).toBeVisible();
    await expect(page.getByText("Draft outline", { exact: true })).toBeVisible();
    await expect(page.getByText("Review outline", { exact: true })).toBeVisible();
    // When the duplicate is deleted, then its API record disappears but the source survives.
    await page.getByRole("button", { name: "Delete", exact: true }).click();
    await network.perform({ method: "DELETE", path: `/api/plans/${duplicate.id}`, status: 200 }, () =>
        page.getByRole("dialog", { name: "Delete plan" }).getByRole("button", { name: "Delete" }).click()
    );
    await expect(page).toHaveURL(/\/plans\/?$/);
    const missing = await page.request.get(`${network.origin}/api/plans/${duplicate.id}`);
    expect(missing.status()).toBe(404);
    await page.goto(`/plans/${source.id}`);
    await expect(page.getByRole("heading", { level: 1, name: title })).toBeVisible();
});
