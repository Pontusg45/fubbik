import assert from "node:assert/strict";
import { test } from "node:test";

import { checkMigration, validateMigration } from "./check-rust-migration.mjs";

test("the checked-in Rust migration contract covers every legacy source", () => {
    // Given the checked-in migration manifest and source trees
    // When the complete migration contract is checked
    const result = checkMigration();

    // Then each tracked area has a non-empty, exhaustive inventory
    assert.deepEqual(result.totals, {
        backendRoutes: 52,
        cliCommands: 63,
        mcpSources: 12,
    });
});

test("an untracked legacy source fails the contract", () => {
    // Given a manifest with an empty tracked inventory
    const manifest = {
        version: 1,
        objective: "test",
        areas: {
            cli: {
                inventory: { root: "apps/cli/src/commands", suffix: ".ts", excludeSuffix: ".test.ts" },
                statuses: { pending: [] },
                exitCriterion: "all commands are classified"
            }
        },
        runtimeDependencies: [],
        cutoverGates: ["test"]
    };

    // When it is compared with the repository
    const errors = validateMigration(manifest);

    // Then the omitted sources are reported
    assert.ok(errors.some((error) => error.includes("untracked sources")));
});
