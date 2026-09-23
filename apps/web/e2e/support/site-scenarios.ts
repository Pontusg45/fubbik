import { expect, type APIRequestContext, type Locator, type Page } from "@playwright/test";

import { reportStep } from "./reporting";

export type SiteApi = { readonly request: APIRequestContext; readonly origin: string };

export async function apiJson<T>(
    site: SiteApi,
    method: "get" | "post" | "put" | "patch" | "delete",
    path: string,
    data?: unknown
): Promise<T> {
    const response = await site.request[method](`${site.origin}${path}`, ...(data === undefined ? [] : [{ data }]));
    expect(response.ok(), `${method.toUpperCase()} ${path}: ${await response.text()}`).toBeTruthy();
    return response.json() as Promise<T>;
}

export async function seedChunk(site: SiteApi, title: string, fields: Record<string, unknown> = {}) {
    return apiJson<{ id: string }>(site, "post", "/api/chunks", { title, content: `${title} content`, ...fields });
}

export async function seedSpace(site: SiteApi, name: string) {
    return apiJson<{ id: string }>(site, "post", "/api/spaces", { name });
}

export async function seedWorkspace(site: SiteApi, name: string) {
    return apiJson<{ id: string }>(site, "post", "/api/workspaces", { name });
}

export async function seedWorkspaceWithSpace(site: SiteApi, workspaceName: string, spaceName: string) {
    const workspace = await seedWorkspace(site, workspaceName);
    const space = await seedSpace(site, spaceName);
    await apiJson(site, "post", `/api/workspaces/${workspace.id}/spaces`, { spaceId: space.id });
    return { workspace, space };
}

export async function workspaceSpaces(site: SiteApi, workspaceId: string) {
    const workspace = await apiJson<{ spaces: Array<{ id: string }> }>(site, "get", `/api/workspaces/${workspaceId}`);
    return workspace.spaces;
}

export async function seedConnection(site: SiteApi, sourceId: string, targetId: string) {
    return apiJson<{ id: string }>(site, "post", "/api/connections", { sourceId, targetId, relation: "related_to" });
}

export async function seedRequirement(site: SiteApi, title: string) {
    const created = await apiJson<{ requirement: { id: string } }>(site, "post", "/api/requirements", {
        title,
        steps: [
            { keyword: "given", text: "a saved chunk" },
            { keyword: "when", text: "coverage is inspected" },
            { keyword: "then", text: "the link is visible" }
        ]
    });
    return { id: created.requirement.id, title };
}

export async function setRequirementChunks(site: SiteApi, requirementId: string, chunkIds: string[]) {
    return apiJson(site, "put", `/api/requirements/${requirementId}/chunks`, { chunkIds });
}

export async function seedCoveredChunk(site: SiteApi, title: string) {
    const chunk = await seedChunk(site, title);
    const requirement = await seedRequirement(site, `Coverage requirement ${crypto.randomUUID().slice(0, 6)}`);
    await setRequirementChunks(site, requirement.id, [chunk.id]);
    return { chunk, requirement };
}

export async function openSite(page: Page, path: string) {
    await page.goto(path);
    await page.waitForLoadState("networkidle");
}

export function given<T>(page: Page, title: string, action: () => Promise<T>) {
    return reportStep(`Given ${title}`, page, action);
}

export function when<T>(target: Page | Locator, title: string, action: () => Promise<T>) {
    return reportStep(`When ${title}`, target, action);
}

export function then<T>(page: Page, title: string, action: () => Promise<T>) {
    return reportStep(`Then ${title}`, page, action);
}
