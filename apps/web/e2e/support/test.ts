import { test as base } from "@playwright/test";

import { AuthScreen } from "./screens/auth";
import { ChunkEditor } from "./screens/chunks";
import { FeatureDialog } from "./screens/features";
import { FubbikUI } from "./ui";

export interface FubbikFixtures {
    ui: FubbikUI;
    screens: { auth: AuthScreen; chunks: ChunkEditor; feature: FeatureDialog };
}
export const test = base.extend<FubbikFixtures>({
    ui: async ({ page }, use) => {
        await use(new FubbikUI(page));
    },
    screens: async ({ page, ui }, use) => {
        await use({ auth: new AuthScreen(page, ui), chunks: new ChunkEditor(page, ui), feature: new FeatureDialog(ui) });
    }
});
export { expect } from "@playwright/test";
export { defineForm } from "./ui";
