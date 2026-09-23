import type { Page } from "@playwright/test";
import { reportStep } from "../reporting";

import { defineForm, type FubbikUI } from "../ui";

export interface Credentials {
    readonly email: string;
    readonly password: string;
}
export interface Registration extends Credentials {
    readonly name: string;
}
export class AuthScreen {
    readonly signInForm;
    readonly signUpForm;
    constructor(
        private readonly page: Page,
        private readonly ui: FubbikUI,
        private readonly apiOrigin: string
    ) {
        const form = ui.within(page.locator("form"));
        this.signInForm = defineForm({ email: form.input("Email"), password: form.input("Password") });
        this.signUpForm = defineForm({ name: form.input("Name"), email: form.input("Email"), password: form.input("Password") });
    }
    async openSignUp() {
        await this.page.goto("/login");
        // Preserve the existing SSR suite's readiness wait in one place.
        await this.page.waitForLoadState("networkidle");
    }
    async signUp(user: Registration) {
        await reportStep("Register Fubbik account", this.page, async () => {
            await this.openSignUp();
            await this.signUpForm.fill({ name: user.name, email: user.email, password: user.password });
            await this.ui.within(this.page.locator("form")).button("Sign Up").click();
            await this.page.waitForURL("**/dashboard", { timeout: 15_000 });
        });
    }
    async signIn(user: Credentials) {
        await reportStep("Sign in to Fubbik", this.page, async () => {
            await this.openSignUp();
            await this.ui.button("Already have an account? Sign In").click();
            await this.signInForm.fill({ email: user.email, password: user.password });
            await this.ui.within(this.page.locator("form")).button("Sign In").click();
            await this.page.waitForURL("**/dashboard", { timeout: 15_000 });
        });
    }
    async session() {
        return this.page.evaluate(async origin => {
            const response = await fetch(`${origin}/api/auth/get-session`, { credentials: "include" });
            return { status: response.status, body: await response.json() };
        }, this.apiOrigin);
    }
    async signOut(name: string) {
        await reportStep("Sign out of Fubbik", this.page, async () => {
            await this.ui.dropdownMenu(name).choose("Sign Out");
            await this.page.waitForURL("/");
        });
    }
}
