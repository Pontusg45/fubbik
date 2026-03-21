import { describe, expect, it } from "vitest";

import { parseDependencies } from "./detect-deps";

describe("parseDependencies", () => {
    describe("package.json", () => {
        it("extracts dependency names", () => {
            const content = JSON.stringify({
                name: "my-app",
                dependencies: {
                    react: "^18.0.0",
                    "@acme/auth": "1.0.0",
                    lodash: "4.17.21"
                },
                devDependencies: {
                    vitest: "^1.0.0"
                }
            });
            const deps = parseDependencies("package.json", content);
            expect(deps).toEqual(["react", "@acme/auth", "lodash"]);
        });

        it("returns empty array when no dependencies", () => {
            const content = JSON.stringify({ name: "empty-pkg" });
            expect(parseDependencies("package.json", content)).toEqual([]);
        });

        it("returns empty array for invalid JSON", () => {
            expect(parseDependencies("package.json", "not json")).toEqual([]);
        });
    });

    describe("go.mod", () => {
        it("extracts module paths from require block", () => {
            const content = `module example.com/myapp

go 1.21

require (
	github.com/gin-gonic/gin v1.9.1
	github.com/jackc/pgx/v5 v5.5.0
	golang.org/x/sync v0.6.0
)
`;
            const deps = parseDependencies("go.mod", content);
            expect(deps).toEqual([
                "github.com/gin-gonic/gin",
                "github.com/jackc/pgx/v5",
                "golang.org/x/sync"
            ]);
        });

        it("returns empty array when no require block", () => {
            const content = `module example.com/myapp\n\ngo 1.21\n`;
            expect(parseDependencies("go.mod", content)).toEqual([]);
        });
    });

    describe("unsupported files", () => {
        it("returns empty array for unknown filenames", () => {
            expect(parseDependencies("Cargo.toml", "")).toEqual([]);
            expect(parseDependencies("requirements.txt", "")).toEqual([]);
        });
    });
});
