import { expect, type Page } from "@playwright/test";
import { reportStep } from "../reporting";

import type { ApiTraffic } from "../network";
import { defineForm, type FubbikUI } from "../ui";

export interface ChunkDraft {
    readonly title: string;
    readonly content: string;
}
export interface DecisionContext {
    readonly alternatives: readonly string[];
    readonly consequences: string;
}
export interface ChunkReference {
    readonly id: string;
    readonly url: string;
    readonly path: `/api/chunks/${string}`;
}

export class ChunkEditor {
    readonly form;
    readonly type;
    readonly tags;
    readonly decisionContext;
    readonly deleteDialog;
    readonly contextDrawer;
    constructor(
        private readonly page: Page,
        private readonly ui: FubbikUI,
        private readonly network: ApiTraffic
    ) {
        this.type = ui.nativeSelect("Type", {
            note: "note",
            document: "document",
            reference: "reference",
            schema: "schema",
            checklist: "checklist"
        });
        const tagInput = page.getByLabel("Tags", { exact: true });
        this.tags = ui.tagInput(tagInput, {
            normalize: value => value.trim().toLowerCase(),
            chip: value => tagInput.locator("xpath=ancestor::div[1]").getByText(`${value} ×`, { exact: true })
        });
        this.form = defineForm({
            title: ui.input("Title"),
            content: ui.textarea(page.getByPlaceholder("Write your content...", { exact: true }))
        });
        this.decisionContext = defineForm({ alternatives: ui.input("Alternatives Considered"), consequences: ui.textarea("Consequences") });
        this.deleteDialog = ui.dialog("Delete chunk");
        this.contextDrawer = ui.sheet("More context");
    }
    current(): ChunkReference {
        const url = new URL(this.page.url());
        const match = /^\/chunks\/([^/]+)(?:\/edit)?$/.exec(url.pathname);
        if (!match?.[1] || match[1] === "new") throw new Error("Expected a chunk detail or edit page.");
        return { id: decodeURIComponent(match[1]), url: `${url.origin}/chunks/${match[1]}`, path: `/api/chunks/${match[1]}` };
    }
    async createChunk(values: ChunkDraft) {
        return reportStep("Create a chunk through the editor", this.page, async () => {
            await this.openNew();
            await this.form.fill(values);
            return this.createAndOpen();
        });
    }
    async createAndOpen() {
        const response = await this.network.perform({ method: "POST", path: "/api/chunks", status: 201 }, () => this.create());
        const created = await response.json();
        expect(created.id, "Created chunk ID").toEqual(expect.any(String));
        await this.page.waitForURL(new URL(`/chunks/${encodeURIComponent(created.id)}`, this.page.url()).href, { timeout: 15_000 });
        await expect(this.page.getByRole("heading", { level: 1 })).toBeVisible();
        return this.current();
    }
    async saveAndOpen() {
        const chunk = this.current();
        await this.network.perform({ method: "PATCH", path: chunk.path, status: 200 }, () => this.save());
        await this.page.waitForURL(chunk.url, { timeout: 15_000 });
        await expect(this.page.getByRole("heading", { level: 1 })).toBeVisible();
        return chunk;
    }
    async expectDetails(values: ChunkDraft) {
        await expect(this.page.getByRole("heading", { level: 1, name: values.title, exact: true })).toBeVisible();
        await expect(this.page.getByText(values.content, { exact: true })).toBeVisible();
    }
    async reloadAndExpect(values: ChunkDraft) {
        await this.page.reload();
        await this.expectDetails(values);
    }
    async openEdit() {
        await this.ui.link("Edit").click();
        await this.waitUntilReady();
    }
    async openContext() {
        await this.contextDrawer.open(this.ui.button(/More context/));
        await this.contextDrawer.button(/^Context/).click();
        return this.contextDrawer;
    }
    async expectDecisionContext(values: DecisionContext) {
        return reportStep("Inspect chunk decision context", this.page, async () => {
            const drawer = await this.openContext();
            await Promise.all(
                [...values.alternatives, values.consequences].map(text =>
                    expect(drawer.root.getByText(text, { exact: true })).toBeVisible()
                )
            );
            return drawer;
        });
    }
    async requestDelete() {
        await this.ui.dropdownMenu("More actions").choose("Delete");
        await this.deleteDialog.expectVisible();
        return this.deleteDialog;
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
        await this.tags.add(tag);
    }
    async setDecisionContext(values: DecisionContext) {
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
