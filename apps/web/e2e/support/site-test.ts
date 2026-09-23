import type { Registration } from "./screens/auth";
import type { SiteApi } from "./site-scenarios";
import { test, testAccount } from "./test";

/** Site tests start with an isolated signed-in account and its authenticated API context. */
export const siteTest = test.extend<{ account: Registration; site: SiteApi }>({
    account: [
        async ({ screens }, use) => {
            const account = testAccount();
            await screens.auth.signUp(account);
            await use(account);
        },
        { auto: true }
    ],
    site: async ({ page, network }, use) => {
        await use({ request: page.request, origin: network.origin });
    }
});
