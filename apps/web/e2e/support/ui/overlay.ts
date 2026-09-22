import { expect, test, type Locator } from "@playwright/test";

import { Button, Checkbox, Radio, Surface, named, type Name } from "./core";

export interface OverlayOptions {
    readonly content?: Locator;
    readonly role: "dialog" | "listbox" | "menu" | "tooltip";
    readonly activation?: "click" | "hover" | "focus" | "contextmenu";
}
function idSelector(id: string) {
    return `[id="${id.replace(/["\\\n\r\f]/g, c => `\\${c.codePointAt(0)?.toString(16)} `)}"]`;
}
export class Overlay {
    readonly trigger: Button;
    private resolved: Locator | undefined;
    constructor(
        trigger: Locator,
        private readonly options: OverlayOptions
    ) {
        this.trigger = new Button(trigger);
    }
    async open(): Promise<Surface> {
        return test.step("Open overlay", async () => {
            if (this.resolved && (await this.resolved.isVisible())) return new Surface(this.resolved);
            // Capture only frame/relationship metadata before modal aria-hiding makes role lookup unavailable.
            const handle = await this.trigger.root.elementHandle();
            if (!handle) throw new Error("Overlay trigger is detached.");
            try {
                const frame = await handle.ownerFrame();
                if (!frame) throw new Error("Overlay frame is detached.");
                if ((await handle.getAttribute("aria-expanded")) !== "true") {
                    switch (this.options.activation) {
                        case "hover":
                            await this.trigger.root.hover();
                            break;
                        case "focus":
                            await this.trigger.root.focus();
                            break;
                        case "contextmenu":
                            await this.trigger.root.click({ button: "right" });
                            break;
                        default:
                            await this.trigger.click();
                    }
                }
                const relationship =
                    (await handle.getAttribute("aria-controls")) ??
                    (this.options.role === "tooltip" ? await handle.getAttribute("aria-describedby") : null);
                const ids = relationship?.trim().split(/\s+/);
                if (!this.options.content && ids && ids.length !== 1)
                    throw new Error("Overlay has multiple relationships; supply content explicitly.");
                const id = ids?.[0];
                const content = this.options.content ?? (id ? frame.locator(idSelector(id)) : frame.getByRole(this.options.role));
                await expect(content, "Overlay content must resolve uniquely; supply an explicit content locator.").toHaveCount(1);
                await expect(content).toBeVisible();
                this.resolved = content;
                return new Surface(content);
            } finally {
                await handle.dispose();
            }
        });
    }
    async close() {
        if (!this.resolved) throw new Error("Open the overlay before closing it.");
        if (await this.resolved.isVisible()) await this.resolved.press("Escape");
        await expect(this.resolved).toBeHidden();
    }
}
export class Dialog extends Surface {
    async open(trigger: Button) {
        await trigger.click();
        await this.expectVisible();
    }
    button(name: Name) {
        return new Button(this.root.getByRole("button", named(name)));
    }
    async close(name: Name = "Close") {
        await this.button(name).click();
        await this.expectHidden();
    }
    async dismiss() {
        await this.root.press("Escape");
        await this.expectHidden();
    }
}
export class Menu extends Surface {
    async choose(name: Name) {
        await this.root.getByRole("menuitem", named(name)).click();
    }
    checkbox(name: Name) {
        return new Checkbox(this.root.getByRole("menuitemcheckbox", named(name)));
    }
    radio(name: Name) {
        return new Radio(this.root.getByRole("menuitemradio", named(name)));
    }
}
export class DropdownMenu {
    private readonly overlay: Overlay;
    constructor(trigger: Locator, options: { readonly content?: Locator } = {}) {
        this.overlay = new Overlay(trigger, { role: "menu", ...options });
    }
    async open() {
        return new Menu((await this.overlay.open()).root);
    }
    async choose(name: Name) {
        await (await this.open()).choose(name);
    }
}
