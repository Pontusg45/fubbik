import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import openapiTS, { astToString, COMMENT_HEADER } from "openapi-typescript";
import { describe, expect, it } from "vitest";

const here = path.dirname(fileURLToPath(import.meta.url));
const schemaPath = path.resolve(here, "../../../openapi.json");
const committedTypesPath = path.join(here, "api-types.ts");

describe("generated API types", () => {
    it("match the committed OpenAPI schema", async () => {
        const generated = COMMENT_HEADER + astToString(await openapiTS(new URL(`file://${schemaPath}`)));
        const committed = readFileSync(committedTypesPath, "utf8");

        expect(committed, "src/api-types.ts is stale; run `pnpm gen:api`").toBe(generated);
    });
});
