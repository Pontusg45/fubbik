/* eslint-disable no-await-in-loop -- Form controls share focus and must be filled sequentially. */
import { reportStep } from "../reporting";

import { fieldValue, validateField, writeField, type FormField } from "./core";

export type FormFields = Readonly<Record<string, FormField>>;
export type FormValues<Fields extends FormFields> = { -readonly [Key in keyof Fields]: Fields[Key][typeof fieldValue] };
type NoExtraKeys<Values, Shape> = Values & Record<Exclude<keyof Values, keyof Shape>, never>;

export function defineForm<const Fields extends FormFields>(fields: Fields) {
    type Values = FormValues<Fields>;
    async function write(values: Partial<Values>, complete: boolean) {
        const keys = Object.keys(values);
        if (complete && Object.keys(fields).some(key => !Object.hasOwn(values, key)))
            throw new TypeError("fill() requires every form field; use patch() for partial updates.");
        // Validate the complete input before performing any browser interaction.
        for (const key of keys) {
            if (!Object.hasOwn(fields, key)) throw new TypeError(`Unknown form field: ${key}`);
            const field = fields[key];
            if (!field) throw new TypeError(`Missing form binding: ${key}`);
            field[validateField](values[key as keyof Values]);
        }
        for (const key of keys) {
            const field = fields[key];
            if (!field) throw new TypeError(`Missing form binding: ${key}`);
            await reportStep(`Fill field: ${key}`, field.root, () => field[writeField](values[key as keyof Values]));
        }
    }
    return {
        fields,
        async fill<const Input extends Values>(values: NoExtraKeys<Input, Values>) {
            await write(values, true);
        },
        async patch<const Input extends Partial<Values>>(values: NoExtraKeys<Input, Values>) {
            await write(values, false);
        }
    };
}
