import { expect, test, testAccount } from "./support/test";

const TEST_USER = testAccount();

test.describe.serial("Auth flow", () => {
    test("sign up creates account and redirects to dashboard", async ({ page, screens }) => {
        // Given a new account and an unauthenticated browser.
        // When the user signs up.
        await screens.auth.signUp(TEST_USER);

        // Then the dashboard shows the authenticated user.
        await expect(page.getByRole("button", { name: TEST_USER.name })).toBeVisible();
    });

    test("sign out returns to home", async ({ page, screens }) => {
        // Given an authenticated user.
        await screens.auth.signIn(TEST_USER);

        // When the user signs out.
        await screens.auth.signOut(TEST_USER.name);
        // The landing page header is hidden entirely (see __root.tsx `isLanding`),
        // so there is no "Sign In" link to assert against here — that string
        // doesn't exist anywhere in routes/index.tsx. Assert instead that the
        // authenticated user-name button (the same locator line 36 uses to prove
        // a *successful* sign-in) is gone, which is what actually demonstrates
        // sign-out took effect. Do not restore the "Sign In" link check.
        // Then the authenticated user control disappears.
        await expect(page.getByRole("button", { name: TEST_USER.name })).not.toBeVisible();
    });

    test("sign in with existing account works", async ({ page, screens }) => {
        // Given an existing account and an unauthenticated browser.
        // When the user signs in.
        await screens.auth.signIn(TEST_USER);
        // Then the dashboard identifies that user.
        await expect(page.getByRole("button", { name: TEST_USER.name })).toBeVisible();
    });

    test("authenticated API returns real user", async ({ screens }) => {
        // Given an authenticated browser session.
        await screens.auth.signIn(TEST_USER);

        // Use the browser context (which has the session cookie) to call the API
        // When the browser requests its session.
        const response = await screens.auth.session();

        // Then the API returns the authenticated user.
        expect(response.status).toBe(200);
        expect(response.body.user.email).toBe(TEST_USER.email);
        expect(response.body.user.name).toBe(TEST_USER.name);
    });
});
