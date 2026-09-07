#!/usr/bin/env node
/**
 * Ensures `legacyApi` is only imported from approved call sites while the
 * Node→Rust port is incomplete. See `src/utils/legacy-api-routes.ts`.
 */
import { readFileSync, readdirSync, statSync } from "node:fs";
import { join, relative } from "node:path";

const ROOT = join(import.meta.dirname, "..");
const SRC = join(ROOT, "src");

const ALLOWED = new Set();

const USES_LEGACY = /\bimport\b[^;\n]*\blegacyApi\b|\blegacyApi\s*\./;

function walk(dir) {
    const out = [];
    for (const entry of readdirSync(dir)) {
        const full = join(dir, entry);
        if (statSync(full).isDirectory()) out.push(...walk(full));
        else if (/\.(tsx?)$/.test(entry)) out.push(full);
    }
    return out;
}

const offenders = [];
for (const file of walk(SRC)) {
    const rel = relative(SRC, file);
    const text = readFileSync(file, "utf8");
    if (!USES_LEGACY.test(text)) continue;
    if (rel === "utils/legacy-api-routes.ts") continue;
    if (!ALLOWED.has(rel)) offenders.push(rel);
}

if (offenders.length > 0) {
    console.error("Unexpected legacyApi imports:\n" + offenders.map(f => `  - src/${f}`).join("\n"));
    console.error("\nAdd to scripts/check-legacy-api.mjs only if the route is still Node-only.");
    process.exit(1);
}

console.log("legacyApi imports OK (" + ALLOWED.size + " allowed files)");
