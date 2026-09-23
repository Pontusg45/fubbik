import type { ApiTraffic } from "../network";
import type { FubbikUI } from "../ui";
import { defineForm } from "../ui";

export class FeatureDialog {
    readonly dialog;
    readonly form;
    constructor(
        private readonly ui: FubbikUI,
        private readonly network: ApiTraffic
    ) {
        this.dialog = ui.dialog("New Feature");
        this.form = defineForm({ name: ui.within(this.dialog.root).input("Name") });
    }
    async createFeature(name: string) {
        await this.open();
        await this.form.fill({ name });
        await this.network.perform({ method: "POST", path: "/api/features", status: 201 }, () => this.submit());
        await this.dialog.expectHidden();
    }
    async open() {
        await this.dialog.open(this.ui.button("New Feature"));
    }
    async submit() {
        await this.dialog.button("Create").click();
    }
}
