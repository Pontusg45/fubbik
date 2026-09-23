/* eslint-disable no-await-in-loop -- Each sort click depends on the state produced by the previous click. */
import { expect, type Locator } from "@playwright/test";
import { reportStep } from "../reporting";

import { Button, Checkbox, Surface, named, type Name } from "./core";

export interface Column {
    readonly label: Name;
    readonly sortable?: true;
}
export type Columns = Readonly<Record<string, Column>>;
export type ColumnKey<Schema extends Columns> = Extract<keyof Schema, string>;
export type SortableKey<Schema extends Columns> = {
    [Key in ColumnKey<Schema>]: Schema[Key] extends { readonly sortable: true } ? Key : never;
}[ColumnKey<Schema>];
export interface TableOptions<Schema extends Columns> {
    readonly columns: Schema;
}
export type RowKey<Schema extends Columns> = { readonly column: ColumnKey<Schema>; readonly value: Name };
function exactText(value: Name) {
    return value instanceof RegExp ? value : new RegExp(`^${value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}$`);
}
export class Table<Schema extends Columns> extends Surface {
    constructor(
        root: Locator,
        readonly options: TableOptions<Schema>
    ) {
        super(root);
    }
    private column(key: ColumnKey<Schema>): Column {
        if (!Object.hasOwn(this.options.columns, key)) throw new TypeError(`Unknown column: ${key}`);
        const column = this.options.columns[key];
        if (!column) throw new TypeError(`Missing column: ${key}`);
        return column;
    }
    async columnIndex(key: ColumnKey<Schema>) {
        const header = this.root.getByRole("columnheader", named(this.column(key).label));
        await expect(header).toHaveCount(1);
        // Read the index of this actual header; never use an unchecked findIndex/-1.
        const index = await header.evaluate(node => (node as HTMLTableCellElement).cellIndex);
        if (!Number.isInteger(index) || index < 0) throw new Error("Expected a semantic table header cell.");
        return index;
    }
    row(key: RowKey<Schema>) {
        this.column(key.column);
        return new TableRow(this, key);
    }
    async expectRowCount(count: number) {
        await expect(this.root.locator("tbody").getByRole("row")).toHaveCount(count);
    }
    async sortBy(key: SortableKey<Schema>, direction: "ascending" | "descending") {
        const column = this.column(key);
        if (!column.sortable) throw new TypeError(`Column is not sortable: ${key}`);
        await reportStep(`Sort table by ${key}: ${direction}`, this.root.getByRole("columnheader", named(column.label)), async () => {
            const header = this.root.getByRole("columnheader", named(column.label));
            const button = new Button(header.getByRole("button"));
            for (let attempts = 0; attempts < 3; attempts++) {
                const previous = await header.getAttribute("aria-sort");
                if (previous === direction) return;
                await button.click();
                await expect.poll(() => header.getAttribute("aria-sort")).not.toBe(previous);
            }
            await expect(header).toHaveAttribute("aria-sort", direction);
        });
    }
}
export class TableRow<Schema extends Columns> {
    constructor(
        private readonly table: Table<Schema>,
        private readonly key: RowKey<Schema>
    ) {}
    private async resolve() {
        const index = await this.table.columnIndex(this.key.column);
        const matchingCell = this.table.root
            .page()
            .getByRole("cell")
            .nth(index)
            .filter({ hasText: exactText(this.key.value) });
        const row = this.table.root.locator("tbody").getByRole("row").filter({ has: matchingCell });
        await expect(row, `Row key must match exactly one rendered row in column ${this.key.column}`).toHaveCount(1);
        return row;
    }
    async expectCellText(column: ColumnKey<Schema>, value: Name) {
        await expect((await this.resolve()).getByRole("cell").nth(await this.table.columnIndex(column))).toHaveText(value);
    }
    async select(selected = true) {
        await new Checkbox((await this.resolve()).getByRole("checkbox")).set(selected);
    }
    async expectSelected(selected = true) {
        await new Checkbox((await this.resolve()).getByRole("checkbox")).expectChecked(selected);
    }
}
