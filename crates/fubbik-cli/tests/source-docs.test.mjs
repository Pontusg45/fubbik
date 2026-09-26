import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, copyFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const rustBinary = process.env.FUBBIK_TEST_BINARY ?? fileURLToPath(new URL("../../../target/debug/fubbik", import.meta.url));

test("Javadoc extracts overloads, tags and source locations while excluding private members", () => {
    // Given
    const temporary = mkdtempSync(join(tmpdir(), "fubbik-doclet-test-"));
    try {
        copyFileSync(new URL("./fixtures/legacy-source-docs/FubbikDoclet.java", import.meta.url), join(temporary, "FubbikDoclet.java"));
        // When
        const result = spawnSync(
            "node",
            [
                fileURLToPath(new URL("./fixtures/legacy-source-docs/extract.mjs", import.meta.url)),
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
                    fileURLToPath(new URL("./fixtures/legacy-source-docs/extract.mjs", import.meta.url)),
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

for (const fixtureName of ["source-docs", "source-docs-forms"]) {
    test(`native JavaScript extraction preserves the legacy JSDoc contract for ${fixtureName}`, () => {
        // Given a JavaScript fixture and both extractors
        const temporary = mkdtempSync(join(tmpdir(), "fubbik-docs-differential-"));
        const fixture = fileURLToPath(new URL(`./fixtures/${fixtureName}`, import.meta.url));
        try {
            // When the legacy JSDoc path and shipped Rust CLI extract the same source
            const legacy = spawnSync("node", [
                fileURLToPath(new URL("./fixtures/legacy-source-docs/extract.mjs", import.meta.url)),
                fixture, "javascript", "fixture", temporary
            ], { encoding: "utf8" });
            const native = spawnSync(rustBinary, [
                "docs", "extract", fixture, "--language", "javascript", "--project", "fixture", "--preview"
            ], { encoding: "utf8", env: { ...process.env, PATH: "/usr/bin:/bin" } });

            // Then stable symbol keys, signatures, documentation, and locations agree exactly
            assert.equal(legacy.status, 0, legacy.stderr);
            assert.equal(native.status, 0, native.stderr);
            assert.deepEqual(JSON.parse(native.stdout).symbols, JSON.parse(legacy.stdout).symbols);
        } finally {
            rmSync(temporary, { recursive: true, force: true });
        }
    });
}

test("native TypeScript extraction preserves the TypeDoc symbol contract", () => {
    // Given the TypeDoc entry-point fixture and both extractors
    const temporary = mkdtempSync(join(tmpdir(), "fubbik-typedoc-differential-"));
    const fixture = fileURLToPath(new URL("./fixtures/source-docs", import.meta.url));
    try {
        // When the legacy and native paths extract the same typed function
        const legacy = spawnSync("node", [
            fileURLToPath(new URL("./fixtures/legacy-source-docs/extract.mjs", import.meta.url)),
            fixture, "typescript", "fixture", temporary
        ], { encoding: "utf8" });
        const native = spawnSync(rustBinary, [
            "docs", "extract", fixture, "--language", "typescript", "--project", "fixture", "--preview"
        ], { encoding: "utf8", env: { ...process.env, PATH: "/usr/bin:/bin" } });

        // Then symbol keys, signatures, comments, and locations agree exactly
        assert.equal(legacy.status, 0, legacy.stderr);
        assert.equal(native.status, 0, native.stderr);
        assert.deepEqual(JSON.parse(native.stdout).symbols, JSON.parse(legacy.stdout).symbols);
    } finally {
        rmSync(temporary, { recursive: true, force: true });
    }
});

test("native Java extraction preserves the Javadoc symbol contract", () => {
    // Given the Java overload fixture and the legacy Doclet
    const temporary = mkdtempSync(join(tmpdir(), "fubbik-javadoc-differential-"));
    const fixture = fileURLToPath(new URL("./fixtures/source-docs", import.meta.url));
    try {
        copyFileSync(new URL("./fixtures/legacy-source-docs/FubbikDoclet.java", import.meta.url), join(temporary, "FubbikDoclet.java"));

        // When the legacy and native paths extract the same declarations
        const legacy = spawnSync("node", [
            fileURLToPath(new URL("./fixtures/legacy-source-docs/extract.mjs", import.meta.url)),
            fixture, "java", "fixture", temporary
        ], { encoding: "utf8" });
        const native = spawnSync(rustBinary, [
            "docs", "extract", fixture, "--language", "java", "--project", "fixture", "--preview"
        ], { encoding: "utf8", env: { ...process.env, PATH: "/usr/bin:/bin" } });

        // Then overload keys, signatures, comments, and source lines agree exactly
        assert.equal(legacy.status, 0, legacy.stderr);
        assert.equal(native.status, 0, native.stderr);
        assert.deepEqual(JSON.parse(native.stdout).symbols, JSON.parse(legacy.stdout).symbols);
    } finally {
        rmSync(temporary, { recursive: true, force: true });
    }
});

test("failed source parsing never produces a complete manifest", () => {
    // Given
    const temporary = mkdtempSync(join(tmpdir(), "fubbik-docs-invalid-"));
    try {
        writeFileSync(join(temporary, "invalid.js"), "/** Broken declaration. */\nexport function (");
        // When
        const result = spawnSync(
            "node",
            [fileURLToPath(new URL("./fixtures/legacy-source-docs/extract.mjs", import.meta.url)), temporary, "javascript", "fixture", temporary],
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
