import type { Page } from "@playwright/test";

import { siteTest as test } from "./support/site-test";
import { expect } from "./support/test";

async function createTemplate(page: Page, origin: string, name: string) {
    const response = await page.request.post(`${origin}/api/templates`, {
        data: { name, type: "note", content: "# Template body", description: "Reusable template" }
    });
    expect(response.status()).toBe(201);
    return (await response.json()) as { id: string; name: string };
}

function templateRow(page: Page, name: string) {
    return page.getByRole("button", { name, exact: true }).locator("xpath=ancestor::div[contains(@class,'items-start')][1]");
}

test("a tag created in the page appears in the list", async ({ page, network }) => {
    // Given the Tags page.
    await page.goto("/tags");
    await page.waitForLoadState("networkidle");
    const name = `ui-tag-${crypto.randomUUID().slice(0, 6)}`;
    // When a new tag is submitted.
    await page.getByRole("button", { name: "New tag" }).click();
    await page.getByPlaceholder("Tag name").fill(name);
    await network.perform({ method: "POST", path: "/api/tags", status: 201 }, () =>
        page.getByPlaceholder("Tag name").locator("xpath=ancestor::form[1]").getByRole("button", { name: "Create" }).click()
    );
    // Then the tag is visible after reload.
    await page.reload();
    await expect(page.getByText(name, { exact: true })).toBeVisible();
});

test("tag search narrows the displayed tags", async ({ page, network }) => {
    // Given two distinct tags.
    await page.request.post(`${network.origin}/api/tags`, { data: { name: "alpha-search-tag" } });
    await page.request.post(`${network.origin}/api/tags`, { data: { name: "beta-search-tag" } });
    await page.goto("/tags");
    await page.waitForLoadState("networkidle");
    // When the user searches for alpha.
    await page.getByPlaceholder(/Filter tags or types/).fill("alpha-search");
    // Then beta is excluded.
    await expect(page.getByText("alpha-search-tag", { exact: true })).toBeVisible();
    await expect(page.getByText("beta-search-tag", { exact: true })).toHaveCount(0);
});

test("the unused-tag filter hides a tag attached to a chunk", async ({ page, network }) => {
    // Given one used tag and one unused tag.
    await page.request.post(`${network.origin}/api/chunks`, { data: { title: "Tagged content", tags: ["used-filter-tag"] } });
    await page.request.post(`${network.origin}/api/tags`, { data: { name: "unused-filter-tag" } });
    await page.goto("/tags");
    await page.waitForLoadState("networkidle");
    // When Unused is selected.
    await page.getByRole("button", { name: /Unused/ }).click();
    // Then only the unused tag remains.
    await expect(page.getByText("unused-filter-tag", { exact: true })).toBeVisible();
    await expect(page.getByText("used-filter-tag", { exact: true })).toHaveCount(0);
});

test("a tag type can be created from the sidebar", async ({ page, network }) => {
    // Given the Tag Types sidebar.
    await page.goto("/tags");
    await page.waitForLoadState("networkidle");
    const name = `type-${crypto.randomUUID().slice(0, 6)}`;
    // When a new type is submitted.
    await page.getByRole("heading", { name: "Tag Types" }).locator("xpath=../..").getByRole("button").click();
    await page.getByPlaceholder("Type name").fill(name);
    await network.perform({ method: "POST", path: "/api/tag-types", status: 201 }, () =>
        page.getByPlaceholder("Type name").locator("xpath=ancestor::form[1]").getByRole("button", { name: "Create" }).click()
    );
    // Then the sidebar shows the persisted type.
    await page.reload();
    await expect(page.getByText(name, { exact: true })).toBeVisible();
});

test("a tag type can be renamed", async ({ page, network }) => {
    // Given a saved tag type.
    const name = `old-type-${crypto.randomUUID().slice(0, 6)}`;
    const created = await page.request.post(`${network.origin}/api/tag-types`, { data: { name, color: "#123456" } });
    expect(created.status()).toBe(201);
    const id = ((await created.json()) as { id: string }).id;
    await page.goto("/tags");
    await page.waitForLoadState("networkidle");
    // When its edit control is used.
    await page.getByText(name, { exact: true }).locator("xpath=..").locator("button").first().click();
    await page.getByPlaceholder("Type name").fill("renamed-type");
    await network.perform({ method: "PATCH", path: `/api/tag-types/${id}`, status: 200 }, () =>
        page.getByPlaceholder("Type name").locator("xpath=ancestor::form[1]").getByRole("button", { name: "Save" }).click()
    );
    // Then the renamed type persists.
    await page.reload();
    await expect(page.getByText("renamed-type", { exact: true })).toBeVisible();
});

test("a template can be created from the page", async ({ page, network }) => {
    // Given the Templates page.
    await page.goto("/templates");
    await page.waitForLoadState("networkidle");
    const name = `UI template ${crypto.randomUUID().slice(0, 6)}`;
    // When the user fills and creates a template.
    await page.getByRole("button", { name: "New Template" }).click();
    await page.getByPlaceholder("Name", { exact: true }).fill(name);
    await page.getByPlaceholder("Template content...").fill("# New template body");
    await network.perform({ method: "POST", path: "/api/templates", status: 201 }, () =>
        page
            .getByRole("heading", { name: "Create Template" })
            .locator("xpath=ancestor::form[1]")
            .getByRole("button", { name: "Create" })
            .click()
    );
    // Then the template appears after reload.
    await page.reload();
    await expect(page.getByRole("button", { name, exact: true })).toBeVisible();
});

test("template preview shows the stored content", async ({ page, network }) => {
    // Given a saved template.
    const template = await createTemplate(page, network.origin, "Preview template");
    await page.goto("/templates");
    await page.waitForLoadState("networkidle");
    // When the template is opened for preview.
    await page.getByRole("button", { name: template.name, exact: true }).click();
    // Then its full body and description are displayed.
    await expect(page.getByRole("heading", { name: template.name })).toBeVisible();
    await expect(page.locator("pre").filter({ hasText: "# Template body" })).toBeVisible();
    await expect(page.getByText("Reusable template", { exact: true }).last()).toBeVisible();
});

test("duplicating a template creates a second template", async ({ page, network }) => {
    // Given one saved template.
    const template = await createTemplate(page, network.origin, "Duplicate template");
    await page.goto("/templates");
    await page.waitForLoadState("networkidle");
    // When Duplicate is clicked on its row.
    await templateRow(page, template.name).getByRole("button", { name: "Duplicate", exact: true }).click();
    await expect(page.getByPlaceholder("Name", { exact: true })).toHaveValue(`${template.name} (copy)`);
    await network.perform({ method: "POST", path: "/api/templates", status: 201 }, () =>
        page
            .getByRole("heading", { name: "Create Template" })
            .locator("xpath=ancestor::form[1]")
            .getByRole("button", { name: "Create" })
            .click()
    );
    // Then the API contains two templates with distinct IDs.
    const templates = (await (await page.request.get(`${network.origin}/api/templates`)).json()) as Array<{ id: string }>;
    expect(templates).toHaveLength(2);
    expect(new Set(templates.map(item => item.id)).size).toBe(2);
});

test("editing a template changes its persisted content", async ({ page, network }) => {
    // Given one saved template.
    const template = await createTemplate(page, network.origin, "Edit template");
    await page.goto("/templates");
    await page.waitForLoadState("networkidle");
    // When its content is edited and saved.
    await templateRow(page, template.name).getByRole("button", { name: "Edit", exact: true }).click();
    await page.getByPlaceholder("Template content...").fill("# Revised template body");
    await network.perform({ method: "PATCH", path: `/api/templates/${template.id}`, status: 200 }, () =>
        page
            .getByRole("heading", { name: "Edit Template" })
            .locator("xpath=ancestor::form[1]")
            .getByRole("button", { name: "Save" })
            .click()
    );
    // Then the updated content survives reload.
    await page.reload();
    await expect(page.getByText("# Revised template body", { exact: true })).toBeVisible();
});

test("confirmed template deletion removes it", async ({ page, network }) => {
    // Given a saved template.
    const template = await createTemplate(page, network.origin, "Delete template");
    await page.goto("/templates");
    await page.waitForLoadState("networkidle");
    // When the row action and confirmation are clicked.
    await templateRow(page, template.name).getByRole("button", { name: "Delete", exact: true }).click();
    await network.perform({ method: "DELETE", path: `/api/templates/${template.id}`, status: 200 }, () =>
        page.getByRole("dialog", { name: "Delete template" }).getByRole("button", { name: "Delete" }).click()
    );
    // Then the template disappears from the API and UI.
    await page.reload();
    await expect(page.getByRole("button", { name: template.name, exact: true })).toHaveCount(0);
    expect((await (await page.request.get(`${network.origin}/api/templates`)).json()) as Array<unknown>).toHaveLength(0);
});
