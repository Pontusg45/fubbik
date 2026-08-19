// Staleness guard for `api-types.ts`.
//
// `api-types.ts` is generated from the repo-root `openapi.json` by
// `pnpm gen:api` (see `package.json`'s `gen:api` script). Nothing forces
// anyone to re-run that after `openapi.json` changes, so it can silently
// drift — which is exactly what happened: this file went missing the
// entire `documents` domain (~500 lines) for two phases before anyone
// noticed the web app was still calling Node for it.
//
// This test regenerates the types *in memory* (never touching the
// committed file) using the same `openapi-typescript` programmatic API the
// CLI itself calls, with the same default options `pnpm gen:api` uses (no
// flags), and fails loudly if the committed file doesn't match.
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import openapiTS, { astToString, COMMENT_HEADER } from "openapi-typescript";
import { describe, expect, it } from "vitest";

const here = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(here, "../../../../");
const schemaPath = path.join(repoRoot, "openapi.json");
const committedTypesPath = path.join(here, "api-types.ts");

describe("api-types.ts staleness guard", () => {
    it("matches what `pnpm gen:api` would currently generate from openapi.json", async () => {
        const ast = await openapiTS(new URL(`file://${schemaPath}`));
        const generated = `${COMMENT_HEADER}${astToString(ast)}`;
        const committed = readFileSync(committedTypesPath, "utf8");

        expect(
            committed,
            "src/utils/api-types.ts is stale relative to openapi.json — run `pnpm gen:api` (from apps/web) and commit the result."
        ).toBe(generated);
    });
});
