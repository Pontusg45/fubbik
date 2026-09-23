import { AuthScreen } from "./support/screens/auth";
import { expect, test, testAccount } from "./support/test";
import { FubbikUI } from "./support/ui";

test("duplicate email registration keeps the original account", async ({ page, screens }) => {
    // Given an account already registered with this email.
    const owner = testAccount();
    await screens.auth.signUp(owner);
    await screens.auth.signOut(owner.name);
    // When another registration uses the same address.
    await screens.auth.openSignUp();
    await screens.auth.signUpForm.fill({ name: "Imposter", email: owner.email, password: owner.password });
    await page.getByRole("button", { name: "Sign Up", exact: true }).click();
    // Then it cannot create a new session for the imposter.
    await expect(page).toHaveURL(/\/login$/);
    await screens.auth.signIn(owner);
    await expect(page.getByRole("button", { name: owner.name })).toBeVisible();
});

test("incorrect password leaves the browser signed out", async ({ page, screens }) => {
    // Given an existing account with a known password.
    const owner = testAccount();
    await screens.auth.signUp(owner);
    await screens.auth.signOut(owner.name);
    // When the user submits a different password.
    await screens.auth.openSignUp();
    await page.getByRole("button", { name: "Already have an account? Sign In" }).click();
    await screens.auth.signInForm.fill({ email: owner.email, password: "incorrect-password" });
    await page.locator("form").getByRole("button", { name: "Sign In", exact: true }).click();
    // Then authentication does not navigate to the dashboard.
    await expect(page).toHaveURL(/\/login$/);
    await expect(page.getByRole("heading", { name: "Welcome Back" })).toBeVisible();
});

test("signing out removes access to the authenticated chunks endpoint", async ({ page, screens, network }) => {
    // Given a signed-in account.
    const owner = testAccount();
    await screens.auth.signUp(owner);
    expect((await page.request.get(`${network.origin}/api/chunks`)).status()).toBe(200);
    // When the account signs out.
    await screens.auth.signOut(owner.name);
    // Then its browser can no longer read the private API.
    expect((await page.request.get(`${network.origin}/api/chunks`)).status()).toBe(401);
});

test("session and user identity survive a reload", async ({ page, screens }) => {
    // Given a newly signed-in account.
    const owner = testAccount();
    await screens.auth.signUp(owner);
    // When the dashboard is reloaded.
    await page.reload();
    // Then the same user remains authenticated.
    await expect(page.getByRole("button", { name: owner.name })).toBeVisible();
    expect((await screens.auth.session()).body.user.email).toBe(owner.email);
});

test("a second tab shares the authenticated session", async ({ context, screens }) => {
    // Given an authenticated browser context.
    const owner = testAccount();
    await screens.auth.signUp(owner);
    // When a second tab opens the dashboard.
    const second = await context.newPage();
    await second.goto("/dashboard");
    // Then it displays the same account.
    await expect(second.getByRole("button", { name: owner.name })).toBeVisible();
});

const isolatedResources = [
    { label: "plan", path: "/api/plans", body: (name: string) => ({ title: name }), id: (data: any) => data.id, detail: true },
    {
        label: "requirement",
        path: "/api/requirements",
        body: (name: string) => ({
            title: name,
            steps: [
                { keyword: "given", text: "an account" },
                { keyword: "when", text: "it reads" },
                { keyword: "then", text: "access is denied" }
            ]
        }),
        id: (data: any) => data.requirement.id,
        detail: true
    },
    { label: "feature", path: "/api/features", body: (name: string) => ({ name }), id: (data: any) => data.id, detail: true },
    { label: "space", path: "/api/spaces", body: (name: string) => ({ name }), id: (data: any) => data.id, detail: true },
    {
        label: "template",
        path: "/api/templates",
        body: (name: string) => ({ name, type: "note", content: "private template" }),
        id: (data: any) => data.id,
        detail: false
    }
] as const;

for (const resource of isolatedResources) {
    test(`another account cannot read or change a ${resource.label}`, async ({ browser, page, screens, network, baseURL }) => {
        // Given account A owns a named resource.
        await screens.auth.signUp(testAccount({ name: "Owner" }));
        const name = `private-${resource.label}-${crypto.randomUUID().slice(0, 8)}`;
        const created = await page.request.post(`${network.origin}${resource.path}`, { data: resource.body(name) });
        expect(created.ok()).toBeTruthy();
        const id = resource.id(await created.json());
        const secondContext = await browser.newContext({ baseURL: baseURL ?? "http://localhost:3001" });
        try {
            const otherPage = await secondContext.newPage();
            await new AuthScreen(otherPage, new FubbikUI(otherPage), network.origin).signUp(testAccount({ name: "Other" }));
            // When account B requests A's resource and attempts an update.
            const listing = await otherPage.request.get(`${network.origin}${resource.path}`);
            expect(listing.ok()).toBeTruthy();
            expect(JSON.stringify(await listing.json())).not.toContain(id);
            if (resource.detail) {
                const read = await otherPage.request.get(`${network.origin}${resource.path}/${id}`);
                expect(read.status()).toBe(404);
            }
            const change = await otherPage.request.patch(`${network.origin}${resource.path}/${id}`, {
                data: resource.label === "plan" || resource.label === "requirement" ? { title: "intrusion" } : { name: "intrusion" }
            });
            // Then B is denied and A's listing still contains its resource.
            expect(change.status()).toBe(404);
            const original = await page.request.get(`${network.origin}${resource.path}`);
            expect(JSON.stringify(await original.json())).toContain(name);
        } finally {
            await secondContext.close();
        }
    });
}
