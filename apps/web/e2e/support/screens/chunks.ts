import { expect, test, type Page } from "@playwright/test";

import { defineForm, type FubbikUI } from "../ui";

export class ChunkEditor {
    readonly form;
    readonly decisionContext;
    constructor(
        private readonly page: Page,
        private readonly ui: FubbikUI
    ) {
        this.form = defineForm({
            title: ui.input("Title"),
            content: ui.textarea(page.getByPlaceholder("Write your content...", { exact: true }))
        });
        this.decisionContext = defineForm({ alternatives: ui.input("Alternatives Considered"), consequences: ui.textarea("Consequences") });
    }
    async openNew() {
        await this.page.goto("/chunks/new");
        await this.waitUntilReady();
    }
    async waitUntilReady() {
        await this.page.waitForLoadState("networkidle");
        await this.form.fields.title.expectVisible();
    }
    async addTag(tag: string) {
        await test.step("Add Chunk tag", async () => {
            const input = this.ui.input(this.page.locator("#chunk-tags"));
            await input.fill(tag);
            await input.root.press("Enter");
            await expect(this.page.getByText(tag, { exact: true })).toBeVisible();
        });
    }
    async setDecisionContext(values: { alternatives: readonly string[]; consequences: string }) {
        await this.ui.disclosure("Decision Context").expand();
        await this.decisionContext.fill({ alternatives: values.alternatives.join(", "), consequences: values.consequences });
    }
    async create() {
        await this.ui.button(/Create Chunk/).click();
    }
    async save() {
        await this.ui.button("Save Changes").click();
    }
}
