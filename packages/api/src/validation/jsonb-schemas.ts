import { t } from "elysia";

export const scopeSchema = t.Optional(
    t.Record(t.String({ maxLength: 100 }), t.String({ maxLength: 500 }), { maxProperties: 20 })
);

export const aliasesSchema = t.Optional(
    t.Array(t.String({ maxLength: 200 }), { maxItems: 20 })
);

export const alternativesSchema = t.Optional(
    t.Array(t.String({ maxLength: 2000 }), { maxItems: 10 })
);
