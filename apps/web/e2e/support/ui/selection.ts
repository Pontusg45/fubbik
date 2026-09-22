/* eslint-disable no-await-in-loop -- Selection changes share one overlay and must settle sequentially. */
import { expect, test, type Locator } from "@playwright/test";

import { Radio, WritableControl, named, type Name } from "./core";
import { Overlay } from "./overlay";

export type OptionMap = Readonly<Record<string, Name>>;
export type OptionKey<Options extends OptionMap> = Extract<keyof Options, string>;
export interface SelectOptions<Options extends OptionMap> {
    readonly options: Options;
    readonly content?: Locator;
    readonly value?: Locator;
}
function optionKey<Options extends OptionMap>(options: Options, value: unknown): OptionKey<Options> {
    if (typeof value !== "string" || !Object.hasOwn(options, value)) throw new TypeError(`Unknown option key: ${String(value)}`);
    return value as OptionKey<Options>;
}
function optionName<Options extends OptionMap>(options: Options, key: OptionKey<Options>): Name {
    const value = options[key];
    if (value === undefined) throw new TypeError(`Missing option binding: ${key}`);
    return value;
}
export class SingleSelect<Options extends OptionMap> extends WritableControl<OptionKey<Options>> {
    readonly overlay: Overlay;
    constructor(
        root: Locator,
        private readonly options: SelectOptions<Options>
    ) {
        super(root, value => optionKey(options.options, value));
        this.overlay = new Overlay(root, { role: "listbox", ...(options.content ? { content: options.content } : {}) });
    }
    async set(value: OptionKey<Options>) {
        const key = this.validate(value);
        await test.step(`Choose select option: ${key}`, async () => {
            const content = await this.overlay.open();
            await content.root.getByRole("option", named(optionName(this.options.options, key))).click();
        });
    }
    async choose(value: OptionKey<Options>) {
        await this.set(value);
    }
    async expectValue(value: OptionKey<Options>) {
        const key = this.validate(value);
        await expect(this.options.value ?? this.root.locator('[data-slot="select-value"]')).toHaveText(
            optionName(this.options.options, key)
        );
    }
}
export class MultiSelect<Options extends OptionMap> extends WritableControl<readonly OptionKey<Options>[]> {
    readonly overlay: Overlay;
    constructor(
        root: Locator,
        private readonly options: SelectOptions<Options>
    ) {
        super(root, value => {
            if (!Array.isArray(value)) throw new TypeError("Multiselect requires an array of option keys.");
            const keys = value.map(item => optionKey(options.options, item));
            if (new Set(keys).size !== keys.length) throw new TypeError("Multiselect values must be unique.");
            return keys;
        });
        this.overlay = new Overlay(root, { role: "listbox", ...(options.content ? { content: options.content } : {}) });
    }
    async set(values: readonly OptionKey<Options>[]) {
        const selected = new Set(this.validate(values));
        await test.step("Set multiselect options", async () => {
            for (const key of Object.keys(this.options.options))
                await this.setSelected(optionKey(this.options.options, key), selected.has(optionKey(this.options.options, key)));
            await this.overlay.close();
        });
    }
    async setSelected(key: OptionKey<Options>, selected = true) {
        const name = optionName(this.options.options, optionKey(this.options.options, key));
        const content = await this.overlay.open();
        await expect(content.root).toHaveAttribute("aria-multiselectable", "true");
        const option = content.root.getByRole("option", named(name));
        await expect(option).toBeVisible();
        if (((await option.getAttribute("aria-selected")) === "true") !== selected) await option.click();
        await expect(option).toHaveAttribute("aria-selected", String(selected));
    }
    async expectSelected(key: OptionKey<Options>, selected = true) {
        const content = await this.overlay.open();
        await expect(
            content.root.getByRole("option", named(optionName(this.options.options, optionKey(this.options.options, key))))
        ).toHaveAttribute("aria-selected", String(selected));
    }
}
export class RadioGroup<Options extends OptionMap> extends WritableControl<OptionKey<Options>> {
    constructor(
        root: Locator,
        private readonly options: Options
    ) {
        super(root, value => optionKey(options, value));
    }
    option(key: OptionKey<Options>) {
        return new Radio(this.root.getByRole("radio", named(optionName(this.options, optionKey(this.options, key)))));
    }
    async set(key: OptionKey<Options>) {
        await this.option(key).choose();
    }
    async choose(key: OptionKey<Options>) {
        await this.set(key);
    }
    async expectValue(key: OptionKey<Options>) {
        await this.option(key).expectChecked();
    }
}
