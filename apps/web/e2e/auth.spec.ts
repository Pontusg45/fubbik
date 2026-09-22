import { expect, test } from "./support/test";

const TEST_USER = {
    name: "Test User",
    email: `test-${Date.now()}@example.com`,
    password: "testpassword123"
};

test.describe.serial("Auth flow", () => {
    test("sign up creates account and redirects to dashboard", async ({ page, screens }) => {
        await screens.auth.signUp(TEST_USER);

        await expect(page.getByRole("button", { name: TEST_USER.name })).toBeVisible();
    });

    test("sign out returns to home", async ({ page, screens }) => {
        await screens.auth.signIn(TEST_USER);

        await screens.auth.signOut(TEST_USER.name);
        // The landing page header is hidden entirely (see __root.tsx `isLanding`),
        // so there is no "Sign In" link to assert against here — that string
        // doesn't exist anywhere in routes/index.tsx. Assert instead that the
        // authenticated user-name button (the same locator line 36 uses to prove
        // a *successful* sign-in) is gone, which is what actually demonstrates
        // sign-out took effect. Do not restore the "Sign In" link check.
        await expect(page.getByRole("button", { name: TEST_USER.name })).not.toBeVisible();
    });

    test("sign in with existing account works", async ({ page, screens }) => {
        await screens.auth.signIn(TEST_USER);
        await expect(page.getByRole("button", { name: TEST_USER.name })).toBeVisible();
    });

    test("authenticated API returns real user", async ({ page, screens }) => {
        await screens.auth.signIn(TEST_USER);

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
