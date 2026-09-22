import { test, type Page } from "@playwright/test";

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
        private readonly ui: FubbikUI
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
        await test.step("Register Fubbik account", async () => {
            await this.openSignUp();
            await this.signUpForm.fill({ name: user.name, email: user.email, password: user.password });
            await this.ui.within(this.page.locator("form")).button("Sign Up").click();
            await this.page.waitForURL("**/dashboard", { timeout: 15_000 });
        });
    }
    async signIn(user: Credentials) {
        await test.step("Sign in to Fubbik", async () => {
            await this.openSignUp();
            await this.ui.button("Already have an account? Sign In").click();
            await this.signInForm.fill({ email: user.email, password: user.password });
            await this.ui.within(this.page.locator("form")).button("Sign In").click();
            await this.page.waitForURL("**/dashboard", { timeout: 15_000 });
        });
    }
    async signOut(name: string) {
        await test.step("Sign out of Fubbik", async () => {
            await this.ui.dropdownMenu(name).choose("Sign Out");
            await this.page.waitForURL("/");
        });
    }
}
