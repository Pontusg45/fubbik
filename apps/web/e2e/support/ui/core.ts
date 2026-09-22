import { expect, test, type FrameLocator, type Locator, type Page } from "@playwright/test";

export type Name = string | RegExp;
export type Target = Name | Locator;
export type Scope = Page | Locator | FrameLocator;
export const named = (name: Name) => ({ name, exact: true });
export function isName(target: Target): target is Name {
    return typeof target === "string" || target instanceof RegExp;
}
export function byRole(scope: Scope, role: Parameters<Page["getByRole"]>[0], target: Target) {
    return isName(target) ? scope.getByRole(role, named(target)) : target;
}
export function byLabel(scope: Scope, target: Target) {
    return isName(target) ? scope.getByLabel(target, { exact: true }) : target;
}

// Internal protocol for heterogeneous forms. Public consumers use typed set().
export const fieldValue: unique symbol = Symbol("fieldValue");
export const validateField: unique symbol = Symbol("validateField");
export const writeField: unique symbol = Symbol("writeField");
export interface FormField {
    readonly [fieldValue]: unknown;
    [validateField](value: unknown): void;
    [writeField](value: unknown): Promise<void>;
}

export class Surface {
    constructor(readonly root: Locator) {}
    async expectVisible() {
        await expect(this.root).toBeVisible();
    }
    async expectHidden() {
        await expect(this.root).toBeHidden();
    }
    async expectText(text: Name) {
        await expect(this.root).toHaveText(text);
    }
    async expectContainsText(text: Name) {
        await expect(this.root).toContainText(text);
    }
}
export class Control extends Surface {
    async expectDisabled() {
        await expect(this.root).toBeDisabled();
    }
    async expectEnabled() {
        await expect(this.root).toBeEnabled();
    }
}
export class Button extends Control {
    async click() {
        await this.root.click();
    }
    async press(key: string) {
        await this.root.press(key);
    }
}
export class Link extends Surface {
    async click() {
        await this.root.click();
    }
    async expectHref(href: Name) {
        await expect(this.root).toHaveAttribute("href", href);
    }
}
export abstract class WritableControl<Value> extends Control implements FormField {
    declare readonly [fieldValue]: Value;
    constructor(
        root: Locator,
        private readonly parse: (value: unknown) => Value
    ) {
        super(root);
    }
    abstract set(value: Value): Promise<void>;
    [validateField](value: unknown) {
        this.parse(value);
    }
    async [writeField](value: unknown) {
        await this.set(this.parse(value));
    }
    protected validate(value: unknown): Value {
        return this.parse(value);
    }
}
export function stringValue(value: unknown): string {
    if (typeof value !== "string") throw new TypeError("Expected a string field value.");
    return value;
}
export function booleanValue(value: unknown): boolean {
    if (typeof value !== "boolean") throw new TypeError("Expected a boolean field value.");
    return value;
}
export class Input extends WritableControl<string> {
    constructor(root: Locator) {
        super(root, stringValue);
    }
    async set(value: string) {
        await this.root.fill(this.validate(value));
    }
    async fill(value: string) {
        await this.set(value);
    }
    async clear() {
        await this.fill("");
    }
    async expectValue(value: Name) {
        await expect(this.root).toHaveValue(value);
    }
    async expectReadOnly() {
        await expect(this.root).toHaveJSProperty("readOnly", true);
    }
    async expectInvalid(invalid = true) {
        if (invalid) await expect(this.root).toHaveAttribute("aria-invalid", "true");
        else await expect(this.root).not.toHaveAttribute("aria-invalid", /^(true|grammar|spelling)$/);
    }
}
export class BinaryControl extends WritableControl<boolean> {
    constructor(root: Locator) {
        super(root, booleanValue);
    }
    async set(value: boolean) {
        await this.root.setChecked(this.validate(value));
    }
    async setChecked(checked: boolean) {
        await this.set(checked);
    }
    async expectChecked(checked = true) {
        await expect(this.root).toBeChecked({ checked });
    }
}
export class Checkbox extends BinaryControl {
    async expectMixed() {
        await expect(this.root).toBeChecked({ indeterminate: true });
    }
}
export class Switch extends BinaryControl {}
export class Radio extends Control {
    async choose() {
        await this.root.check();
    }
    async expectChecked() {
        await expect(this.root).toBeChecked();
    }
}
export class Disclosure extends Surface {
    readonly trigger: Button;
    constructor(root: Locator) {
        super(root);
        this.trigger = new Button(root);
    }
    async expand() {
        await test.step("Expand disclosure", async () => {
            await this.expectVisible();
            if ((await this.root.getAttribute("aria-expanded")) !== "true") await this.trigger.click();
            await this.expectExpanded();
        });
    }
    async collapse() {
        await this.expectVisible();
        if ((await this.root.getAttribute("aria-expanded")) === "true") await this.trigger.click();
        await this.expectExpanded(false);
    }
    async expectExpanded(expanded = true) {
        await expect(this.root).toHaveAttribute("aria-expanded", String(expanded));
    }
}
