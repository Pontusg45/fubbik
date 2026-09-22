import type { FubbikUI } from "../ui";
import { defineForm } from "../ui";

export class FeatureDialog {
    readonly dialog;
    readonly form;
    constructor(private readonly ui: FubbikUI) {
        this.dialog = ui.dialog("New Feature");
        this.form = defineForm({ name: ui.within(this.dialog.root).input("Name") });
    }
    async open() {
        await this.dialog.open(this.ui.button("New Feature"));
    }
    async submit() {
        await this.dialog.button("Create").click();
    }
}
