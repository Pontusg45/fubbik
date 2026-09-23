import type { Page } from "@playwright/test";

import { siteTest as test } from "./support/site-test";
import { expect } from "./support/test";

type Task = {
    id: string;
    title: string;
    status: string;
    description: string | null;
    acceptanceCriteria: Array<{ text: string; done: boolean }>;
};
type PlanDetail = {
    plan: { id: string; status: string };
    tasks: Task[];
    dependencies: Array<{ id: string; taskId: string; dependsOnTaskId: string }>;
};

async function createPlan(page: Page, origin: string, tasks: Array<{ title: string; acceptanceCriteria?: string[] }> = []) {
    const title = `Plan tasks ${crypto.randomUUID().slice(0, 8)}`;
    const response = await page.request.post(`${origin}/api/plans`, { data: { title, tasks } });
    expect(response.status()).toBe(200);
    const plan = (await response.json()) as { id: string };
    await page.goto(`/plans/${plan.id}`);
    await page.waitForLoadState("networkidle");
    await expect(page.getByRole("heading", { level: 1, name: title })).toBeVisible();
    return { id: plan.id, title };
}

async function detail(page: Page, origin: string, id: string): Promise<PlanDetail> {
    const response = await page.request.get(`${origin}/api/plans/${id}`);
    expect(response.status()).toBe(200);
    return response.json() as Promise<PlanDetail>;
}

function card(page: Page, title: string) {
    return page.getByText(title, { exact: true }).locator("xpath=ancestor::div[contains(@class,'bg-card')][1]");
}

test("adding a task from the plan detail persists it", async ({ page, network }) => {
    // Given a plan with no tasks.
    const plan = await createPlan(page, network.origin);
    // When a new task is added.
    await page.getByRole("button", { name: "Add task" }).click();
    await page.getByPlaceholder(/Task title/).fill("Write release notes");
    await expect(page.getByRole("button", { name: "Add", exact: true }).last()).toBeEnabled();
    await network.perform({ method: "POST", path: `/api/plans/${plan.id}/tasks`, status: 200 }, () =>
        page.getByPlaceholder(/Task title/).press("Enter")
    );
    // Then the task survives a reload.
    await page.reload();
    await expect(page.getByRole("button", { name: "Write release notes", exact: true })).toBeVisible();
    expect((await detail(page, network.origin, plan.id)).tasks.map(task => task.title)).toContain("Write release notes");
});

test("double-clicking a task title allows renaming", async ({ page, network }) => {
    // Given a seeded task.
    const plan = await createPlan(page, network.origin, [{ title: "Old task title" }]);
    // When its title is edited inline.
    await page.getByText("Old task title", { exact: true }).dblclick();
    await page.locator('input[value="Old task title"]').fill("New task title");
    await page.keyboard.press("Enter");
    // Then the new title persists.
    await expect(page.getByText("New task title", { exact: true })).toBeVisible();
    expect((await detail(page, network.origin, plan.id)).tasks[0]?.title).toBe("New task title");
});

test("editing a task description persists Markdown text", async ({ page, network }) => {
    // Given a seeded task.
    const plan = await createPlan(page, network.origin, [{ title: "Describe task" }]);
    // When its details are expanded and description edited.
    const task = card(page, "Describe task");
    await task.getByRole("button", { name: "Toggle details" }).click();
    await task.getByPlaceholder("Add a description (markdown supported)").fill("**Review** before shipping.");
    await task.getByPlaceholder("Add a description (markdown supported)").blur();
    // Then the API retains the description.
    await expect.poll(async () => (await detail(page, network.origin, plan.id)).tasks[0]?.description).toBe("**Review** before shipping.");
});

test("a task status change persists as done", async ({ page, network }) => {
    // Given a pending task.
    const plan = await createPlan(page, network.origin, [{ title: "Finish task" }]);
    // When Done is selected from its status menu.
    await card(page, "Finish task").locator('[title^="Status:"]').click();
    await page.getByRole("option", { name: "Done" }).click();
    // Then the persisted task is done.
    await expect.poll(async () => (await detail(page, network.origin, plan.id)).tasks[0]?.status).toBe("done");
});

test("task completion updates the plan progress count", async ({ page, network }) => {
    // Given a plan with two tasks.
    const plan = await createPlan(page, network.origin, [{ title: "First progress" }, { title: "Second progress" }]);
    // When one task is marked done through its status control.
    await card(page, "First progress").locator('[title^="Status:"]').click();
    await page.getByRole("option", { name: "Done" }).click();
    // Then the header shows one of two tasks completed, including after reload.
    await expect(page.getByText("1/2 tasks", { exact: true })).toBeVisible();
    await page.reload();
    await expect(page.getByText("1/2 tasks", { exact: true })).toBeVisible();
    expect((await detail(page, network.origin, plan.id)).tasks.filter(task => task.status === "done")).toHaveLength(1);
});

test("checking acceptance criteria stores their completion", async ({ page, network }) => {
    // Given a task with one acceptance criterion.
    const plan = await createPlan(page, network.origin, [{ title: "Acceptance task", acceptanceCriteria: ["Tests pass"] }]);
    // When the criterion is checked.
    const task = card(page, "Acceptance task");
    await task.getByRole("button", { name: "Toggle details" }).click();
    await task.getByRole("checkbox", { name: "Tests pass" }).click();
    // Then it remains checked after reload.
    await page.reload();
    await card(page, "Acceptance task").getByRole("button", { name: "Toggle details" }).click();
    await expect(card(page, "Acceptance task").getByRole("checkbox", { name: "Tests pass" })).toBeChecked();
    expect((await detail(page, network.origin, plan.id)).tasks[0]?.acceptanceCriteria[0]?.done).toBe(true);
});

test("a task can depend on another task", async ({ page, network }) => {
    // Given two tasks in one plan.
    const plan = await createPlan(page, network.origin, [{ title: "Prerequisite" }, { title: "Dependent" }]);
    // When the prerequisite is added to the dependent task.
    const dependent = card(page, "Dependent");
    await dependent.getByRole("button", { name: "Toggle details" }).click();
    await dependent.getByRole("combobox").last().click();
    await page.getByRole("option", { name: "Prerequisite" }).click();
    // Then the dependency is stored.
    await expect.poll(async () => (await detail(page, network.origin, plan.id)).dependencies.length).toBe(1);
});

test("a task dependency can be removed", async ({ page, network }) => {
    // Given a dependent task with a prerequisite.
    const plan = await createPlan(page, network.origin, [{ title: "Prerequisite" }, { title: "Dependent" }]);
    const initial = await detail(page, network.origin, plan.id);
    const prerequisite = initial.tasks.find(task => task.title === "Prerequisite")!;
    const dependent = initial.tasks.find(task => task.title === "Dependent")!;
    const added = await page.request.post(`${network.origin}/api/plans/${plan.id}/tasks/${dependent.id}/dependencies`, {
        data: { dependsOnTaskId: prerequisite.id }
    });
    expect(added.status()).toBe(200);
    await page.reload();
    // When the dependency removal control is clicked.
    const task = card(page, "Dependent");
    await task.getByRole("button", { name: "Toggle details" }).click();
    await task.getByRole("button", { name: "Remove dependency" }).click();
    // Then no dependency remains.
    await expect.poll(async () => (await detail(page, network.origin, plan.id)).dependencies.length).toBe(0);
});

test("reordering tasks changes their persisted display order", async ({ page, network }) => {
    // Given three tasks in their original order.
    const plan = await createPlan(page, network.origin, [{ title: "Order one" }, { title: "Order two" }, { title: "Order three" }]);
    const initial = await detail(page, network.origin, plan.id);
    const reordered = [...initial.tasks].reverse();
    // When their order is changed through the reorder endpoint.
    const response = await page.request.post(`${network.origin}/api/plans/${plan.id}/tasks/reorder`, {
        data: { taskIds: reordered.map(task => task.id) }
    });
    expect(response.status()).toBe(200);
    // Then the detail UI and API reflect the new order.
    await page.reload();
    const taskTitles = (await detail(page, network.origin, plan.id)).tasks.map(task => task.title);
    expect(taskTitles).toEqual(reordered.map(task => task.title));
    await expect(page.getByText("Order three", { exact: true })).toBeVisible();
});

test("archiving and unarchiving a plan restores draft status", async ({ page, network }) => {
    // Given a draft plan.
    const plan = await createPlan(page, network.origin);
    // When its Archive action is clicked.
    await network.perform({ method: "PATCH", path: `/api/plans/${plan.id}`, status: 200 }, () =>
        page.getByRole("button", { name: "Archive", exact: true }).click()
    );
    await expect.poll(async () => (await detail(page, network.origin, plan.id)).plan.status).toBe("archived");
    // When Unarchive is clicked, then the plan returns to draft.
    await network.perform({ method: "PATCH", path: `/api/plans/${plan.id}`, status: 200 }, () =>
        page.getByRole("button", { name: "Unarchive", exact: true }).click()
    );
    await expect.poll(async () => (await detail(page, network.origin, plan.id)).plan.status).toBe("draft");
});
