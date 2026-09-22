import type { Locator } from "@playwright/test";

import { Button, Checkbox, Disclosure, Input, Link, Surface, Switch, byLabel, byRole, type Scope, type Target } from "./core";
import { Dialog, DropdownMenu, Overlay, type OverlayOptions } from "./overlay";
import { MultiSelect, RadioGroup, SingleSelect, type OptionMap, type SelectOptions } from "./selection";
import { Table, type Columns, type TableOptions } from "./table";

export class FubbikUI {
    constructor(readonly scope: Scope) {}
    within(scope: Scope) {
        return new FubbikUI(scope);
    }
    button(target: Target) {
        return new Button(byRole(this.scope, "button", target));
    }
    link(target: Target) {
        return new Link(byRole(this.scope, "link", target));
    }
    input(target: Target) {
        return new Input(byLabel(this.scope, target));
    }
    textarea(target: Target) {
        return this.input(target);
    }
    checkbox(target: Target) {
        return new Checkbox(byRole(this.scope, "checkbox", target));
    }
    switch(target: Target) {
        return new Switch(byRole(this.scope, "switch", target));
    }
    radioGroup<const Options extends OptionMap>(target: Target, options: Options) {
        return new RadioGroup(byRole(this.scope, "radiogroup", target), options);
    }
    select<const Options extends OptionMap>(target: Target, options: SelectOptions<Options>) {
        return new SingleSelect(byRole(this.scope, "combobox", target), options);
    }
    multiSelect<const Options extends OptionMap>(target: Target, options: SelectOptions<Options>) {
        return new MultiSelect(byRole(this.scope, "combobox", target), options);
    }
    dialog(target: Target) {
        return new Dialog(byRole(this.scope, "dialog", target));
    }
    sheet(target: Target) {
        return this.dialog(target);
    }
    dropdownMenu(target: Target, options: { readonly content?: Locator } = {}) {
        return new DropdownMenu(byRole(this.scope, "button", target), options);
    }
    disclosure(target: Target) {
        return new Disclosure(byRole(this.scope, "button", target));
    }
    popover(target: Target, options: Omit<OverlayOptions, "role"> = {}) {
        return new Overlay(byRole(this.scope, "button", target), { ...options, role: "dialog" });
    }
    table<const Schema extends Columns>(target: Target, options: TableOptions<Schema>) {
        return new Table(byRole(this.scope, "table", target), options);
    }
    content(root: Locator) {
        return new Surface(root);
    }
    card(root: Locator) {
        return this.content(root);
    }
}
export { defineForm, type FormValues } from "./form";
export type { Name, Scope, Target } from "./core";
export type { OverlayOptions } from "./overlay";
export type { OptionKey, OptionMap, SelectOptions } from "./selection";
export type { Column, ColumnKey, Columns, RowKey, TableOptions } from "./table";
