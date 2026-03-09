import { expect, type Page, test } from "@playwright/test";

const TEST_USER = {
    name: "Test User",
    email: `test-${Date.now()}@example.com`,
    password: "testpassword123"
};

async function waitForHydration(page: Page) {
    // networkidle waits for all scripts to load and execute, ensuring React
    // has hydrated and attached event handlers to the SSR-rendered form
    await page.waitForLoadState("networkidle");
}

async function signIn(page: Page) {
    await page.goto("/login");
    await waitForHydration(page);
    await page.getByRole("button", { name: "Already have an account? Sign In" }).click();
    await page.getByLabel("Email").fill(TEST_USER.email);
    await page.getByLabel("Password").fill(TEST_USER.password);
    await page.locator("form").getByRole("button", { name: "Sign In" }).click();
    await page.waitForURL("**/dashboard", { timeout: 15000 });
}

test.describe.serial("Auth flow", () => {
    test("sign up creates account and redirects to dashboard", async ({ page }) => {
        await page.goto("/login");
        await waitForHydration(page);

        await page.getByLabel("Name").fill(TEST_USER.name);
        await page.getByLabel("Email").fill(TEST_USER.email);
        await page.getByLabel("Password").fill(TEST_USER.password);
        await page.getByRole("button", { name: "Sign Up" }).click();

        await page.waitForURL("**/dashboard", { timeout: 15000 });
        await expect(page.getByRole("button", { name: TEST_USER.name })).toBeVisible();
    });

    test("sign out returns to home", async ({ page }) => {
        await signIn(page);

        await page.getByRole("button", { name: TEST_USER.name }).click();
        await page.getByRole("menuitem", { name: "Sign Out" }).click();

        await page.waitForURL("/");
        await expect(page.getByRole("link", { name: "Sign In" })).toBeVisible();
    });

    test("sign in with existing account works", async ({ page }) => {
        await signIn(page);
        await expect(page.getByRole("button", { name: TEST_USER.name })).toBeVisible();
    });

    test("authenticated API returns real user", async ({ page }) => {
        await signIn(page);

        // Use the browser context (which has the session cookie) to call the API
        const response = await page.evaluate(async () => {
            const res = await fetch("http://localhost:3000/api/me", {
                credentials: "include"
            });
            return { status: res.status, body: await res.json() };
        });

        expect(response.status).toBe(200);
        expect(response.body.user.email).toBe(TEST_USER.email);
        expect(response.body.user.name).toBe(TEST_USER.name);
    });
});
