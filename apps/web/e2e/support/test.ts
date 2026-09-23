import { test as base } from "@playwright/test";

import { ApiTraffic } from "./network";
import { AuthScreen } from "./screens/auth";
import { ChunkEditor } from "./screens/chunks";
import { FeatureDialog } from "./screens/features";
import { SpaceScreen } from "./screens/spaces";
import { FubbikUI } from "./ui";

export interface FubbikFixtures {
    ui: FubbikUI;
    apiOrigin: string;
    network: ApiTraffic;
    screens: { auth: AuthScreen; chunks: ChunkEditor; feature: FeatureDialog; spaces: SpaceScreen };
}
export const test = base.extend<FubbikFixtures>({
    apiOrigin: ["http://localhost:3100", { option: true }],
    network: async ({ page, apiOrigin }, use) => {
        const network = new ApiTraffic(page, apiOrigin);
        try {
            await use(network);
        } finally {
            network.dispose();
        }
    },
    ui: async ({ page }, use) => {
        await use(new FubbikUI(page));
    },
    screens: async ({ page, ui, network }, use) => {
        await use({
            auth: new AuthScreen(page, ui, network.origin),
            chunks: new ChunkEditor(page, ui, network),
            feature: new FeatureDialog(ui, network),
            spaces: new SpaceScreen(page, network)
        });
    }
});
export { expect } from "@playwright/test";
export { defineForm } from "./ui";

export { testAccount } from "./data";
export { reportStep } from "./reporting";
