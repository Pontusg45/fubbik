import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, copyFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

test("Javadoc extracts overloads, tags and source locations while excluding private members", () => {
    // Given
    const temporary = mkdtempSync(join(tmpdir(), "fubbik-doclet-test-"));
    try {
        copyFileSync(new URL("../src/source-docs/FubbikDoclet.java", import.meta.url), join(temporary, "FubbikDoclet.java"));
        // When
        const result = spawnSync(
            "node",
            [
                fileURLToPath(new URL("../src/source-docs/extract.mjs", import.meta.url)),
                fileURLToPath(new URL("./fixtures/source-docs", import.meta.url)),
                "java",
                "fixture",
                temporary
            ],
            { encoding: "utf8" }
        );
        // Then
        assert.equal(result.status, 0, result.stderr);
        const manifest = JSON.parse(result.stdout);
        const methods = manifest.symbols.filter(s => s.key.includes("#find("));
        assert.equal(methods.length, 2);
        assert.notEqual(methods[0].key, methods[1].key);
        assert.ok(methods.every(s => s.path === "Lookup.java" && s.line > 0 && s.documentation.includes("@return")));
        assert.ok(!manifest.symbols.some(s => s.key.includes("internal")));
    } finally {
        rmSync(temporary, { recursive: true, force: true });
    }
});

for (const language of ["javascript", "typescript"]) {
    test(`${language} preserves parameters, returns and examples`, () => {
        // Given
        const temporary = mkdtempSync(join(tmpdir(), "fubbik-docs-test-"));
        try {
            // When
            const result = spawnSync(
                "node",
                [
                    fileURLToPath(new URL("../src/source-docs/extract.mjs", import.meta.url)),
                    fileURLToPath(new URL("./fixtures/source-docs", import.meta.url)),
                    language,
                    "fixture",
                    temporary
                ],
                { encoding: "utf8" }
            );
            // Then
            assert.equal(result.status, 0, result.stderr);
            const manifest = JSON.parse(result.stdout);
            assert.equal(manifest.symbols.length, 1);
            const symbol = manifest.symbols[0];
            assert.ok(symbol.documentation.includes("lookup name") || symbol.documentation.includes("Lookup name"));
            assert.match(symbol.documentation, /eturns/);
            assert.match(symbol.documentation, /xample/);
            assert.ok(symbol.line > 0);
        } finally {
            rmSync(temporary, { recursive: true, force: true });
        }
    });
}

test("failed source parsing never produces a complete manifest", () => {
    // Given
    const temporary = mkdtempSync(join(tmpdir(), "fubbik-docs-invalid-"));
    try {
        writeFileSync(join(temporary, "invalid.js"), "/** Broken declaration. */\nexport function (");
        // When
        const result = spawnSync(
            "node",
            [fileURLToPath(new URL("../src/source-docs/extract.mjs", import.meta.url)), temporary, "javascript", "fixture", temporary],
            { encoding: "utf8" }
        );
        // Then
        assert.notEqual(result.status, 0);
        assert.equal(result.stdout, "");
        assert.match(result.stderr, /failed/);
    } finally {
        rmSync(temporary, { recursive: true, force: true });
    }
});
