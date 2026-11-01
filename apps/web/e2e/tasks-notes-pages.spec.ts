import { apiJson, openSite, seedChunk, seedSpace } from "./support/site-scenarios";
import { siteTest as test } from "./support/site-test";
import { expect } from "./support/test";

test("Tasks page creates and completes standalone work", async ({ page, site }) => {
    // Given an ordinary in-progress plan with a task.
    const plan = await apiJson<{ id: string }>(site, "post", "/api/plans", {
        title: "Plan work should stay separate",
        tasks: [{ title: "Plan-only task" }]
    });
    await apiJson(site, "patch", `/api/plans/${plan.id}`, { status: "in_progress" });

    // When the signed-in user opens the Tasks page.
    await openSite(page, "/tasks");
    await expect(page.getByRole("heading", { name: "Tasks", level: 1 })).toBeVisible();
    await expect(page.getByRole("link", { name: "Plan work should stay separate" })).toHaveCount(0);

    // When a task is created.
    const title = `Review release ${crypto.randomUUID().slice(0, 8)}`;
    await page.getByRole("textbox", { name: "Task title" }).fill(title);
    await page.getByRole("textbox", { name: "Task description" }).fill("Check the release notes");
    await page.getByLabel("Priority").selectOption("high");
    await page.getByRole("button", { name: "Add task" }).click();

    // Then it is linked to its plan and persisted by the API.
    const taskLink = page.getByRole("link", { name: title });
    await expect(taskLink).toBeVisible();
    await expect(page.getByText("Check the release notes")).toBeVisible();
    const tasks = await apiJson<Array<{ id: string; title: string; metadata: { taskQueue?: boolean } }>>(site, "get", "/api/tasks");
    expect(tasks.map(task => task.title)).toContain(title);
    expect(tasks.map(task => task.title)).not.toContain("Plan work should stay separate");
    expect(tasks.find(task => task.title === title)?.metadata.taskQueue).toBe(true);
    expect(tasks.find(task => task.title === title)?.metadata).toMatchObject({ priority: "high" });
    await expect(taskLink).toHaveAttribute("href", `/plans/${tasks.find(task => task.title === title)?.id}`);

    // When it is completed, it leaves the active list.
    await page.getByRole("button", { name: `Complete ${title}` }).click();
    await expect(taskLink).toHaveCount(0);
    await page.getByRole("button", { name: "Completed", exact: true }).click();
    await expect(taskLink).toBeVisible();
    await expect(page.getByRole("button", { name: `Complete ${title}` })).toHaveCount(0);
    const completed = await apiJson<Array<{ title: string }>>(site, "get", "/api/tasks?status=completed");
    expect(completed.map(task => task.title)).toContain(title);
});

test("Notes page filters note chunks and searches their content", async ({ page, site }) => {
    // Given a note and another chunk type.
    const title = `Idea ${crypto.randomUUID().slice(0, 8)}`;
    const note = await seedChunk(site, title, { type: "note", content: "A memorable lighthouse idea" });
    await seedChunk(site, "Unrelated document", { type: "document", content: "A memorable lighthouse document" });

    // When the Notes page opens and the user searches.
    await openSite(page, "/notes");
    await expect(page.getByRole("heading", { name: "Notes", level: 1 })).toBeVisible();
    await expect(page.getByRole("link", { name: title })).toHaveAttribute("href", `/chunks/${note.id}`);
    await expect(page.getByRole("link", { name: "Unrelated document" })).toHaveCount(0);
    await page.getByRole("searchbox", { name: "Search notes" }).fill("lighthouse");

    // Then the search is shareable and survives a reload.
    await expect(page).toHaveURL(/\/notes\/?\?q=lighthouse/);
    await page.reload();
    await expect(page.getByRole("searchbox", { name: "Search notes" })).toHaveValue("lighthouse");
    await expect(page.getByRole("link", { name: title })).toBeVisible();
    await expect(page.getByRole("link", { name: "Unrelated document" })).toHaveCount(0);
    await expect(page.getByRole("link", { name: "New note" })).toHaveAttribute("href", "/chunks/new");
});

test("New note belongs to the selected space", async ({ page, site }) => {
    // Given an active space on the Notes page.
    const space = await seedSpace(site, `Notes space ${crypto.randomUUID().slice(0, 8)}`);
    await page.evaluate(id => localStorage.setItem("active-space", id), space.id);
    await openSite(page, "/notes");

    // When a note is created from the page.
    await page.getByRole("link", { name: "New note" }).first().click();
    const title = `Space note ${crypto.randomUUID().slice(0, 8)}`;
    await page.getByRole("textbox", { name: "Title", exact: true }).fill(title);
    await page.getByRole("button", { name: "Create Chunk" }).click();
    await expect(page.getByRole("heading", { name: title, level: 1 })).toBeVisible({ timeout: 30_000 });

    // Then its saved space assignment matches the active space.
    const chunkId = page.url().split("/").at(-1);
    const detail = await apiJson<{ spaces: Array<{ id: string }> }>(site, "get", `/api/chunks/${chunkId}`);
    expect(detail.spaces.map(entry => entry.id)).toContain(space.id);
    await openSite(page, "/notes");
    await expect(page.getByRole("link", { name: title })).toBeVisible();
});
