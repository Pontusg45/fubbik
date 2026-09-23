import type { Page } from "@playwright/test";

import { siteTest as test } from "./support/site-test";
import { expect } from "./support/test";

const steps = [
    { keyword: "given", text: "a signed-in user" },
    { keyword: "when", text: "the user opens a requirement" },
    { keyword: "then", text: "the detail appears" }
];

async function seedRequirement(page: Page, origin: string, fields: Record<string, unknown> = {}) {
    const title = typeof fields.title === "string" ? fields.title : `Requirement ${crypto.randomUUID().slice(0, 8)}`;
    const response = await page.request.post(`${origin}/api/requirements`, { data: { title, steps, ...fields } });
    expect(response.status()).toBe(201);
    const body = (await response.json()) as { requirement: { id: string } };
    return { id: body.requirement.id, title };
}

async function openRequirement(page: Page, id: string) {
    await page.goto(`/requirements/${id}`);
    await page.waitForLoadState("networkidle");
}

async function fillSteps(page: Page) {
    const inputs = page.getByPlaceholder("Step description...");
    await steps.reduce((previous, step, index) => previous.then(() => inputs.nth(index).fill(step.text)), Promise.resolve());
}

test("a valid Given/When/Then requirement can be created in the form", async ({ page, network }) => {
    // Given a new requirement form with all three step kinds.
    await page.goto("/requirements/new");
    await page.waitForLoadState("networkidle");
    const title = `UI requirement ${crypto.randomUUID().slice(0, 8)}`;
    await page.getByLabel("Title", { exact: true }).fill(title);
    await fillSteps(page);
    // When the requirement is submitted.
    const response = await network.perform({ method: "POST", path: "/api/requirements", status: 201 }, () =>
        page.getByRole("button", { name: "Create Requirement" }).click()
    );
    // Then detail contains the title and all steps after reload.
    const id = ((await response.json()) as { requirement: { id: string } }).requirement.id;
    await page.waitForURL(`**/requirements/${id}`);
    await page.reload();
    await expect(page.getByRole("heading", { level: 1, name: title })).toBeVisible();
    await Promise.all(steps.map(step => expect(page.getByText(step.text, { exact: true })).toBeVisible()));
});

test("an empty requirement title blocks creation", async ({ page, network }) => {
    // Given complete steps but no title.
    await page.goto("/requirements/new");
    await page.waitForLoadState("networkidle");
    await fillSteps(page);
    const writes = network.record({ method: "POST", path: "/api/requirements" });
    // When Create is clicked.
    await page.getByRole("button", { name: "Create Requirement" }).click();
    // Then the form shows an error without a write.
    await expect(page.getByText("Title is required", { exact: true })).toBeVisible();
    expect(writes.requests).toHaveLength(0);
});

test("an empty requirement step blocks creation", async ({ page, network }) => {
    // Given a title with untouched, empty steps.
    await page.goto("/requirements/new");
    await page.waitForLoadState("networkidle");
    await page.getByLabel("Title", { exact: true }).fill("Incomplete steps");
    const writes = network.record({ method: "POST", path: "/api/requirements" });
    // When Create is clicked.
    await page.getByRole("button", { name: "Create Requirement" }).click();
    // Then step validation prevents the POST.
    await expect(page.getByText("Step text is required").first()).toBeVisible();
    expect(writes.requests).toHaveLength(0);
});

test("priority selected in the create form persists", async ({ page, network }) => {
    // Given a complete requirement form.
    await page.goto("/requirements/new");
    await page.waitForLoadState("networkidle");
    await page.getByLabel("Title", { exact: true }).fill("Must-have requirement");
    await fillSteps(page);
    // When Must priority is selected and the form is saved.
    await page.getByLabel("Priority").selectOption("must");
    const response = await network.perform({ method: "POST", path: "/api/requirements", status: 201 }, () =>
        page.getByRole("button", { name: "Create Requirement" }).click()
    );
    // Then the stored requirement has Must priority.
    const id = ((await response.json()) as { requirement: { id: string } }).requirement.id;
    const stored = await page.request.get(`${network.origin}/api/requirements/${id}`);
    expect((await stored.json()).priority).toBe("must");
});

test("editing a requirement description persists", async ({ page, network }) => {
    // Given a saved requirement.
    const requirement = await seedRequirement(page, network.origin);
    await openRequirement(page, requirement.id);
    // When its description changes in edit mode.
    await page.getByRole("button", { name: "Edit", exact: true }).click();
    await page.getByPlaceholder("Describe the requirement...").fill("Edited requirement description.");
    await network.perform({ method: "PATCH", path: `/api/requirements/${requirement.id}`, status: 200 }, () =>
        page.getByRole("button", { name: "Save Changes" }).click()
    );
    // Then the description appears after reload.
    await page.reload();
    await expect(page.getByText("Edited requirement description.", { exact: true })).toBeVisible();
});

test("adding a requirement step persists its keyword and text", async ({ page, network }) => {
    // Given a saved requirement with three steps.
    const requirement = await seedRequirement(page, network.origin);
    await openRequirement(page, requirement.id);
    // When an And step is added in edit mode.
    await page.getByRole("button", { name: "Edit", exact: true }).click();
    await page.getByRole("button", { name: "Add step" }).click();
    await page.getByPlaceholder("Step description...").last().fill("another condition holds");
    await network.perform({ method: "PATCH", path: `/api/requirements/${requirement.id}`, status: 200 }, () =>
        page.getByRole("button", { name: "Save Changes" }).click()
    );
    // Then the new step appears in detail and stored data.
    await page.reload();
    await expect(page.getByText("another condition holds", { exact: true })).toBeVisible();
    const stored = await page.request.get(`${network.origin}/api/requirements/${requirement.id}`);
    expect((await stored.json()).steps).toHaveLength(4);
});

test("a chunk can be linked to a requirement", async ({ page, network }) => {
    // Given a requirement and a saved chunk.
    const requirement = await seedRequirement(page, network.origin);
    const created = await page.request.post(`${network.origin}/api/chunks`, { data: { title: "Linkable chunk" } });
    expect(created.status()).toBe(201);
    await openRequirement(page, requirement.id);
    // When the chunk is chosen in edit mode.
    await page.getByRole("button", { name: "Edit", exact: true }).click();
    await page.getByPlaceholder("Search chunks to link...").fill("Linkable chunk");
    await page.getByRole("button", { name: "Linkable chunk" }).click();
    await network.perform({ method: "PUT", path: `/api/requirements/${requirement.id}/chunks`, status: 200 }, () =>
        page.getByRole("button", { name: "Save Changes" }).click()
    );
    // Then the chunk appears in linked content after reload.
    await page.reload();
    await expect(page.getByRole("link", { name: "Linkable chunk" })).toBeVisible();
});

test("a linked chunk can be removed from a requirement", async ({ page, network }) => {
    // Given a requirement with a linked chunk.
    const requirement = await seedRequirement(page, network.origin);
    const created = await page.request.post(`${network.origin}/api/chunks`, { data: { title: "Removable linked chunk" } });
    const chunk = (await created.json()) as { id: string };
    await page.request.put(`${network.origin}/api/requirements/${requirement.id}/chunks`, { data: { chunkIds: [chunk.id] } });
    await openRequirement(page, requirement.id);
    // When its chip is removed in edit mode.
    await page.getByRole("button", { name: "Edit", exact: true }).click();
    await page.getByText("Removable linked chunk x", { exact: true }).click();
    await network.perform({ method: "PUT", path: `/api/requirements/${requirement.id}/chunks`, status: 200 }, () =>
        page.getByRole("button", { name: "Save Changes" }).click()
    );
    // Then the linked chunk is absent after reload.
    await page.reload();
    await expect(page.getByRole("link", { name: "Removable linked chunk" })).toHaveCount(0);
});

test("requirement list search narrows to a matching title", async ({ page, network }) => {
    // Given two requirements with distinct titles.
    const target = await seedRequirement(page, network.origin, { title: "Search target requirement" });
    await seedRequirement(page, network.origin, { title: "Other requirement" });
    await page.goto("/requirements");
    await page.waitForLoadState("networkidle");
    // When the user searches for the target.
    await page.getByPlaceholder(/Search requirements/).fill("Search target");
    // Then only the matching requirement is shown.
    await expect(page.getByText(target.title, { exact: true })).toBeVisible();
    await expect(page.getByText("Other requirement", { exact: true })).toHaveCount(0);
});

test("confirmed requirement deletion removes its API record", async ({ page, network }) => {
    // Given a saved requirement.
    const requirement = await seedRequirement(page, network.origin);
    await openRequirement(page, requirement.id);
    // When deletion is confirmed.
    await page.getByRole("button", { name: "Delete Requirement" }).click();
    await network.perform({ method: "DELETE", path: `/api/requirements/${requirement.id}`, status: 200 }, () =>
        page.getByRole("dialog", { name: "Delete requirement" }).getByRole("button", { name: "Delete" }).click()
    );
    // Then the user returns to the list and the resource is gone.
    await expect(page).toHaveURL(/\/requirements\/?$/);
    expect((await page.request.get(`${network.origin}/api/requirements/${requirement.id}`)).status()).toBe(404);
});
