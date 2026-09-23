import { describe, expect, it } from "vitest";

import { assertSchemaParity, compareSchemas, type SchemaTable } from "./schema-parity";

const expected: SchemaTable[] = [
    {
        name: "plan",
        columns: [
            { name: "id", sqlType: "text", notNull: true },
            { name: "description", sqlType: "text", notNull: false }
        ]
    }
];

describe("compareSchemas", () => {
    it("reports missing tables and column mismatches together", () => {
        // Given
        const actual: SchemaTable[] = [
            {
                name: "plan",
                columns: [
                    { name: "id", sqlType: "text", notNull: false },
                    { name: "description", sqlType: "jsonb", notNull: false },
                    { name: "unexpected", sqlType: "text", notNull: false }
                ]
            },
            { name: "sql_only", columns: [] }
        ];

        // When the operation is evaluated by the assertion.
        // Then
        expect(compareSchemas(expected, actual)).toEqual([
            "plan.description type: Drizzle=text SQL=jsonb",
            "plan.id nullability: Drizzle=not null SQL=nullable",
            "plan.unexpected exists only in SQL",
            "sql_only exists only in SQL"
        ]);
    });

    it("accepts equivalent schemas regardless of declaration order", () => {
        // Given the inline inputs and test fixtures.
        // When the operation is evaluated by the assertion.
        // Then
        expect(compareSchemas(expected, [{ name: "plan", columns: [...expected[0]!.columns].reverse() }])).toEqual([]);
    });

    it("allows only explicitly documented migration differences", () => {
        // Given the inline inputs and test fixtures.
        // When the operation is evaluated by the assertion.
        // Then
        expect(() =>
            assertSchemaParity(
                ["legacy exists only in Drizzle"],
                [{ issue: "legacy exists only in Drizzle", reason: "removed after cutover" }]
            )
        ).not.toThrow();
        expect(() => assertSchemaParity(["new drift"], [])).toThrow("new drift");
    });
});
