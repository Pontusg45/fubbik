import { apiJson, given, openSite, seedChunk, seedConnection, then, when } from "./support/site-scenarios";
import { siteTest as test } from "./support/site-test";
import { expect } from "./support/test";

async function matrixSite(page: Parameters<typeof openSite>[0], origin: string) {
    const name = `Journey matrix ${crypto.randomUUID().slice(0, 8)}`;
    const matrix = await apiJson<{ id: string }>({ request: page.request, origin }, "post", "/api/matrices", { name, layer: "invariant" });
    await openSite(page, `/matrices/${matrix.id}`);
    await expect(page.getByRole("heading", { name })).toBeVisible();
    return matrix;
}

async function graphChunks(page: Parameters<typeof openSite>[0], origin: string, count: number) {
    const site = { request: page.request, origin };
    const tagName = `graph-${crypto.randomUUID().slice(0, 8)}`;
    const tagType = await apiJson<{ id: string }>(site, "post", "/api/tag-types", { name: `Graph type ${tagName}`, color: "#f97316" });
    await apiJson(site, "post", "/api/tags", { name: tagName, tagTypeId: tagType.id });
    return Promise.all(
        Array.from({ length: count }, (_, index) => seedChunk(site, `Graph chunk ${index} ${tagName}`, { tags: [tagName] }))
    );
}

test("graph renders newly connected chunks", async ({ page, network, site }) => {
    // Given two chunks connected in the real API.
    const chunks = await given(page, "two tagged and connected chunks", async () => {
        const [source, target] = await graphChunks(page, network.origin, 2);
        await seedConnection(site, source!.id, target!.id);
        return { source: source!, target: target! };
    });
    // When the graph is opened.
    await when(page, "the graph loads", () => openSite(page, "/graph"));
    // Then the grouped island can be opened to reveal both persisted chunks.
    await then(page, "both graph nodes are visible", async () => {
        await page.locator('.react-flow__node[data-id^="island-"]').first().click();
        await expect(page.locator(`.react-flow__node[data-id="${chunks.source.id}"]`)).toBeVisible();
        await expect(page.locator(`.react-flow__node[data-id="${chunks.target.id}"]`)).toBeVisible();
    });
});

test("graph detail panel opens from a chunk node", async ({ page, network }) => {
    // Given a persisted chunk.
    const chunk = await given(page, "a tagged graph chunk", async () => (await graphChunks(page, network.origin, 1))[0]!);
    await openSite(page, "/graph");
    await page.locator('.react-flow__node[data-id^="island-"]').first().click();
    const node = page.locator(`.react-flow__node[data-id="${chunk.id}"]`);
    // When its node is selected.
    await when(node, "the graph node is selected", () => node.click());
    // Then a detail panel opens for that chunk.
    await then(page, "the selected chunk detail appears", async () => {
        await expect(page.getByText("Graph chunk 0", { exact: false }).first()).toBeVisible();
    });
});

test("matrix list shows a newly created matrix", async ({ page, site }) => {
    // Given a matrix created through the API.
    const name = `Matrix list ${crypto.randomUUID().slice(0, 8)}`;
    await given(page, "a saved matrix", () => apiJson(site, "post", "/api/matrices", { name, layer: "invariant" }));
    // When the list is opened.
    await when(page, "the matrix list loads", () => openSite(page, "/matrices"));
    // Then the matrix is a navigable entry after reload.
    await then(page, "the matrix remains listed", async () => {
        await expect(page.getByText(name, { exact: true })).toBeVisible();
        await page.reload();
        await expect(page.getByText(name, { exact: true })).toBeVisible();
    });
});

test("matrix detail adds a dimension", async ({ page, network, site }) => {
    // Given an empty matrix.
    const matrix = await given(page, "a matrix with one rule", async () => {
        const created = await matrixSite(page, network.origin);
        await apiJson(site, "post", `/api/matrices/${created.id}/rules`, { title: "Stable rule" });
        await page.reload();
        return created;
    });
    const name = `Browser ${crypto.randomUUID().slice(0, 6)}`;
    const input = page.getByPlaceholder("Dimension name...");
    // When a dimension is added in the UI.
    await when(input, "a dimension is added", async () => {
        await input.fill(name);
        await network.perform({ method: "POST", path: `/api/matrices/${matrix.id}/dimensions`, status: 201 }, () =>
            input.locator("xpath=ancestor::form[1]").getByRole("button", { name: "Add" }).click()
        );
    });
    // Then the API and reloaded page contain it.
    await then(page, "the dimension persists", async () => {
        await page.reload();
        await expect(page.getByText(name, { exact: true })).toBeVisible();
        const view = await apiJson<{ dimensions: Array<{ name: string }> }>(site, "get", `/api/matrices/${matrix.id}/view`);
        expect(view.dimensions.map(dimension => dimension.name)).toContain(name);
    });
});

test("matrix detail adds a rule with decision context", async ({ page, network, site }) => {
    // Given an empty matrix.
    const matrix = await given(page, "an empty matrix", () => matrixSite(page, network.origin));
    const title = `Rule ${crypto.randomUUID().slice(0, 6)}`;
    const input = page.getByPlaceholder("Rule title...");
    // When a rule and rationale are submitted.
    await when(input, "a rule with rationale is added", async () => {
        await input.fill(title);
        await page.getByRole("button", { name: "Decision context (optional)" }).click();
        await page.getByPlaceholder("Why does this rule exist?").fill("Protect the domain invariant.");
        await network.perform({ method: "POST", path: `/api/matrices/${matrix.id}/rules`, status: 201 }, () =>
            input.locator("xpath=ancestor::form[1]").getByRole("button", { name: "Add" }).click()
        );
    });
    // Then the rule survives reload.
    await then(page, "the rule persists", async () => {
        await page.reload();
        const view = await apiJson<{ rules: Array<{ title: string }> }>(site, "get", `/api/matrices/${matrix.id}/view`);
        expect(view.rules.map(rule => rule.title)).toContain(title);
    });
});

test("matrix cell creation updates its coverage summary", async ({ page, network, site }) => {
    // Given a matrix with one rule and dimension.
    const matrix = await given(page, "a rule and dimension", async () => {
        const created = await matrixSite(page, network.origin);
        await apiJson(site, "post", `/api/matrices/${created.id}/dimensions`, { name: "API" });
        await apiJson(site, "post", `/api/matrices/${created.id}/rules`, { title: "Stable output" });
        await page.reload();
        return created;
    });
    const cell = page.getByRole("button", { name: "Create cell" });
    // When the empty cell is created.
    await when(cell, "the matrix cell is created", () => cell.click());
    // Then coverage is updated and remains so after reload.
    await then(page, "one cell is specified", async () => {
        await expect(page.getByText("1 total", { exact: true })).toBeVisible();
        await page.reload();
        await expect(page.getByText("1 total", { exact: true })).toBeVisible();
        const view = await apiJson<{ summary: { total: number } }>(site, "get", `/api/matrices/${matrix.id}/view`);
        expect(view.summary.total).toBe(1);
    });
});

test("matrix cell can be removed from its grid", async ({ page, network, site }) => {
    // Given an occupied matrix cell.
    const matrix = await given(page, "an occupied matrix cell", async () => {
        const created = await matrixSite(page, network.origin);
        const dimension = await apiJson<{ id: string }>(site, "post", `/api/matrices/${created.id}/dimensions`, { name: "Web" });
        const rule = await apiJson<{ id: string }>(site, "post", `/api/matrices/${created.id}/rules`, { title: "Readable" });
        await apiJson(site, "put", `/api/matrices/${created.id}/cells`, { ruleId: rule.id, dimensionId: dimension.id });
        await page.reload();
        return created;
    });
    const cell = page.getByRole("button", { name: /specified, 0 requirements/ });
    // When the occupied cell is removed with its context action.
    await when(cell, "the occupied cell is removed", () => cell.click({ button: "right" }));
    // Then the grid and API return to the empty state.
    await then(page, "the cell is absent", async () => {
        await expect(page.getByRole("button", { name: "Create cell" })).toBeVisible();
        const view = await apiJson<{ summary: { total: number } }>(site, "get", `/api/matrices/${matrix.id}/view`);
        expect(view.summary.total).toBe(0);
    });
});
