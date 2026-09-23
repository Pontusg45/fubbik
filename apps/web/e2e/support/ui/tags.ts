import { expect, type Locator } from "@playwright/test";
import { reportStep } from "../reporting";

import { Input, stringValue } from "./core";

export interface TagInputOptions {
    readonly chip: (value: string) => Locator;
    readonly remove?: (value: string) => Locator;
    readonly normalize?: (value: string) => string;
}

// Chips are application-specific; bind their locators explicitly instead of assuming markup.
export class TagInput {
    readonly input: Input;
    constructor(
        root: Locator,
        private readonly options: TagInputOptions
    ) {
        this.input = new Input(root);
    }
    private value(tag: string) {
        const value = this.options.normalize?.(stringValue(tag)) ?? stringValue(tag);
        if (!value.trim()) throw new TypeError("Tag must not be empty.");
        return value;
    }
    async add(tag: string) {
        const value = this.value(tag);
        await reportStep(`Add tag: ${value}`, this.input.root, async () => {
            await this.input.fill(tag);
            await this.input.root.press("Enter");
            await expect(this.options.chip(value)).toHaveCount(1);
            await this.input.expectValue("");
        });
    }
    async remove(tag: string) {
        const value = this.value(tag);
        await reportStep(`Remove tag: ${value}`, (this.options.remove ?? this.options.chip)(value), async () => {
            await (this.options.remove ?? this.options.chip)(value).click();
            await expect(this.options.chip(value)).toHaveCount(0);
        });
    }
    async expectTag(tag: string, present = true) {
        const chip = this.options.chip(this.value(tag));
        if (present) await expect(chip).toHaveCount(1);
        else await expect(chip).toHaveCount(0);
    }
}
