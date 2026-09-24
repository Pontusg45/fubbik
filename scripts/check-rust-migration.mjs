#!/usr/bin/env node

import { existsSync, readFileSync, readdirSync } from "node:fs";
import { dirname, join, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const defaultManifest = join(repoRoot, "migration/rust-migration.json");

function filesBelow(root) {
    const results = [];
    for (const entry of readdirSync(root, { withFileTypes: true })) {
        const path = join(root, entry.name);
        if (entry.isDirectory()) results.push(...filesBelow(path));
        else results.push(path);
    }
    return results;
}

function sourceOf(entry) {
    return typeof entry === "string" ? entry : entry.source;
}

function inventoryFiles(spec, root) {
    const inventoryRoot = join(root, spec.root);
    return filesBelow(inventoryRoot)
        .filter((path) => path.endsWith(spec.suffix))
        .filter((path) => !spec.excludeSuffix || !path.endsWith(spec.excludeSuffix))
        .map((path) => relative(inventoryRoot, path).split(sep).join("/"))
        .sort();
}

function rustToolNames(spec, root) {
    const sourceRoot = join(root, spec.root);
    const files = filesBelow(sourceRoot).filter((path) => path.endsWith(".rs"));
    const names = files.flatMap((path) =>
        [...readFileSync(path, "utf8").matchAll(/\btool\(\s*"([^"]+)"/g)].map((match) => match[1]),
    );
    return names.sort();
}

function compareContract(name, expected, actual, errors) {
    const duplicateExpected = expected.filter((value, index) => expected.indexOf(value) !== index);
    const duplicateActual = actual.filter((value, index) => actual.indexOf(value) !== index);
    if (duplicateExpected.length) {
        errors.push(`${name} has duplicate expected entries: ${[...new Set(duplicateExpected)].join(", ")}`);
    }
    if (duplicateActual.length) {
        errors.push(`${name} has duplicate Rust entries: ${[...new Set(duplicateActual)].join(", ")}`);
    }
    const expectedSet = new Set(expected);
    const actualSet = new Set(actual);
    const missing = expected.filter((value) => !actualSet.has(value));
    const unexpected = actual.filter((value) => !expectedSet.has(value));
    if (missing.length) errors.push(`${name} is missing Rust entries: ${missing.join(", ")}`);
    if (unexpected.length) errors.push(`${name} has untracked Rust entries: ${unexpected.join(", ")}`);
}

export function validateMigration(manifest, root = repoRoot) {
    const errors = [];
    if (manifest.version !== 1) errors.push("manifest.version must be 1");
    if (!manifest.objective) errors.push("manifest.objective is required");

    for (const [areaName, area] of Object.entries(manifest.areas ?? {})) {
        const actual = inventoryFiles(area.inventory, root);
        const claims = [];

        for (const [status, entries] of Object.entries(area.statuses ?? {})) {
            for (const entry of entries) {
                const source = sourceOf(entry);
                if (!source) {
                    errors.push(`${areaName}.${status} contains an entry without a source`);
                    continue;
                }
                claims.push(source);
                if (status === "retire" && typeof entry === "object" && !entry.reason) {
                    errors.push(`${areaName}.${source} is retired without a reason`);
                }
            }
        }

        const duplicates = claims.filter((value, index) => claims.indexOf(value) !== index);
        if (duplicates.length) {
            errors.push(`${areaName} has duplicate claims: ${[...new Set(duplicates)].join(", ")}`);
        }

        const claimSet = new Set(claims);
        const actualSet = new Set(actual);
        const untracked = actual.filter((path) => !claimSet.has(path));
        const missing = claims.filter((path) => !actualSet.has(path));
        if (untracked.length) errors.push(`${areaName} has untracked sources: ${untracked.join(", ")}`);
        if (missing.length) errors.push(`${areaName} claims missing sources: ${missing.join(", ")}`);
        if (!area.exitCriterion) errors.push(`${areaName}.exitCriterion is required`);

        for (const target of area.rustTargets ?? []) {
            if (!existsSync(join(root, target))) {
                errors.push(`${areaName} Rust target does not exist: ${target}`);
            }
        }
    }

    for (const dependency of manifest.runtimeDependencies ?? []) {
        if (!dependency.id || !dependency.status || !dependency.exitCriterion) {
            errors.push("every runtime dependency needs id, status, and exitCriterion");
        }
        if (dependency.evidence && !existsSync(join(root, dependency.evidence))) {
            errors.push(`${dependency.id} evidence does not exist: ${dependency.evidence}`);
        }
    }

    const mcpTools = manifest.executableContracts?.mcpTools;
    if (!mcpTools) {
        errors.push("executableContracts.mcpTools is required");
    } else if (!existsSync(join(root, mcpTools.root))) {
        errors.push(`mcpTools root does not exist: ${mcpTools.root}`);
    } else {
        compareContract("mcpTools", [...mcpTools.expected].sort(), rustToolNames(mcpTools, root), errors);
    }

    if (!(manifest.cutoverGates?.length > 0)) errors.push("cutoverGates must not be empty");
    return errors;
}

export function checkMigration(manifestPath = defaultManifest, root = repoRoot) {
    const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
    const errors = validateMigration(manifest, root);
    if (errors.length) throw new Error(errors.map((error) => `- ${error}`).join("\n"));

    const totals = Object.fromEntries(
        Object.entries(manifest.areas).map(([name, area]) => [
            name,
            Object.values(area.statuses).reduce((sum, entries) => sum + entries.length, 0),
        ]),
    );
    return { version: manifest.version, totals };
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
    try {
        const result = checkMigration(process.argv[2] ? resolve(process.argv[2]) : defaultManifest);
        console.log(
            `Rust migration contract is complete (v${result.version}): ${Object.entries(result.totals)
                .map(([name, count]) => `${name}=${count}`)
                .join(", ")}`,
        );
    } catch (error) {
        console.error(`Rust migration contract check failed:\n${error.message}`);
        process.exitCode = 1;
    }
}
