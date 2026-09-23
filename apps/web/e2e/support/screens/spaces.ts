import { expect, type Page } from "@playwright/test";

import type { ApiTraffic } from "../network";

export class SpaceScreen {
    constructor(
        private readonly page: Page,
        private readonly network: ApiTraffic
    ) {}

    async create(name: string): Promise<string> {
        await this.page.goto("/spaces");
        await this.page.waitForLoadState("networkidle");
        await this.page.getByPlaceholder("Name", { exact: true }).fill(name);
        const response = await this.network.perform({ method: "POST", path: "/api/spaces", status: 201 }, () =>
            this.page.getByRole("button", { name: "Add", exact: true }).click()
        );
        const created = (await response.json()) as { id: string };
        expect(created.id).toEqual(expect.any(String));
        await expect(this.page.getByRole("paragraph").filter({ hasText: name })).toBeVisible();
        return created.id;
    }
}
