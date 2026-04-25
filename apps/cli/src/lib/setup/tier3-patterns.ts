import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";

import type { DiscoveredChunk, Tip } from "./types";

export interface PatternResult {
    chunks: DiscoveredChunk[];
    tips: Tip[];
}

// --- Constants ---

const IGNORE_DIRS = new Set([
    "node_modules",
    ".git",
    ".turbo",
    "dist",
    "build",
    ".next",
    ".output",
    ".cache",
    "coverage",
    ".fubbik",
]);

// --- Types ---

export interface FileDetectionResult {
    found: boolean;
    paths: string[];
}

interface Detector {
    name: string;
    deps: string[];
    findFiles(dir: string): FileDetectionResult;
    buildChunk(dir: string, depName: string, version: string, files: string[]): DiscoveredChunk;
    buildTipFromDep(depName: string): Tip;
    buildTipFromFiles(files: string[]): Tip;
}

// --- Traversal helpers ---

/**
 * Walk a directory up to maxDepth, calling visitor on each entry.
 * Skips IGNORE_DIRS. Stops early if visitor returns true.
 */
export function searchDirs(
    dir: string,
    visitor: (entryPath: string, isDir: boolean, name: string) => boolean | void,
    maxDepth = 4,
    _depth = 0,
): void {
    if (_depth >= maxDepth) return;
    let entries;
    try {
        entries = readdirSync(dir, { withFileTypes: true });
    } catch {
        return;
    }
    for (const entry of entries) {
        if (IGNORE_DIRS.has(entry.name)) continue;
        const full = join(dir, entry.name);
        const stop = visitor(full, entry.isDirectory(), entry.name);
        if (stop) return;
        if (entry.isDirectory()) {
            searchDirs(full, visitor, maxDepth, _depth + 1);
        }
    }
}

/**
 * Find files matching a regex under dir (max depth 4, skips IGNORE_DIRS).
 */
export function findFilesByPattern(dir: string, regex: RegExp): string[] {
    const results: string[] = [];
    searchDirs(dir, (path, isDir, name) => {
        if (!isDir && regex.test(name)) {
            results.push(path);
        }
    });
    return results;
}

// --- Helpers ---

function dirExists(path: string): boolean {
    try {
        return existsSync(path) && statSync(path).isDirectory();
    } catch {
        return false;
    }
}

interface PackageJson {
    dependencies?: Record<string, string>;
    devDependencies?: Record<string, string>;
    peerDependencies?: Record<string, string>;
}

function readPackageJson(dir: string): PackageJson {
    try {
        const content = readFileSync(join(dir, "package.json"), "utf-8");
        return JSON.parse(content) as PackageJson;
    } catch {
        return {};
    }
}

function findDep(pkg: PackageJson, deps: string[]): { name: string; version: string } | null {
    const all: Record<string, string> = {
        ...pkg.dependencies,
        ...pkg.devDependencies,
        ...pkg.peerDependencies,
    };
    for (const dep of deps) {
        if (dep in all) {
            return { name: dep, version: all[dep]! };
        }
    }
    return null;
}

// --- Detectors ---

const DETECTORS: Detector[] = [
    // 1. Route structure
    {
        name: "routing",
        deps: ["elysia", "express", "fastify", "hono", "next", "nuxt", "astro", "remix", "koa"],
        findFiles(dir) {
            const found: string[] = [];
            const candidates = ["routes", "pages", "api"];
            for (const name of candidates) {
                const srcPath = join(dir, "src", name);
                const rootPath = join(dir, name);
                if (dirExists(srcPath)) found.push(srcPath);
                else if (dirExists(rootPath)) found.push(rootPath);
            }
            // Also search recursively if nothing found at common locations
            if (found.length === 0) {
                searchDirs(dir, (path, isDir, name) => {
                    if (isDir && (name === "routes" || name === "pages" || name === "api")) {
                        found.push(path);
                    }
                });
            }
            return { found: found.length > 0, paths: found };
        },
        buildChunk(_dir, depName, version, files) {
            return {
                title: "Routing Conventions",
                content: [
                    `This project uses **${depName}** (${version}) for routing.`,
                    "",
                    "## Route Directories",
                    ...files.map(f => `- \`${f}\``),
                    "",
                    "Add route handler files following the framework's conventions for this directory.",
                ].join("\n"),
                type: "note",
                tags: ["routing", "conventions", depName],
                tier: 3,
                category: "conventions",
                source: `detected:routing:${depName}`,
                appliesTo: files.map(f => `${f}/**`),
            };
        },
        buildTipFromDep(depName) {
            return {
                title: "Consider documenting your routing structure",
                detail: `Found \`${depName}\` in dependencies. Once you add a \`routes/\`, \`pages/\`, or \`api/\` directory, fubbik can auto-generate a routing conventions chunk.`,
            };
        },
        buildTipFromFiles(files) {
            return {
                title: "Consider adding a routing framework",
                detail: `Found route directories (${files.join(", ")}) but no recognized routing framework in package.json. Consider documenting your routing approach manually.`,
            };
        },
    },

    // 2. Test patterns
    {
        name: "testing",
        deps: ["vitest", "jest", "mocha"],
        findFiles(dir) {
            const found: string[] = [];
            // __tests__/ directories
            searchDirs(dir, (path, isDir, name) => {
                if (isDir && name === "__tests__") {
                    found.push(path);
                }
            });
            // *.test.ts / *.spec.ts files (only if no __tests__ dirs found)
            if (found.length === 0) {
                const testFiles = findFilesByPattern(dir, /\.(test|spec)\.(ts|js|tsx|jsx)$/);
                found.push(...testFiles.slice(0, 3));
            }
            return { found: found.length > 0, paths: found };
        },
        buildChunk(_dir, depName, version, files) {
            const runner =
                depName === "vitest" ? "vitest" : depName === "jest" ? "jest" : "mocha";
            return {
                title: "Testing Conventions",
                content: [
                    `This project uses **${depName}** (${version}) for testing.`,
                    "",
                    "## Test Locations",
                    ...files.map(f => `- \`${f}\``),
                    "",
                    `Run tests with \`${runner}\`.`,
                ].join("\n"),
                type: "note",
                tags: ["testing", "conventions", depName],
                tier: 3,
                category: "conventions",
                source: `detected:testing:${depName}`,
            };
        },
        buildTipFromDep(depName) {
            return {
                title: "Consider documenting your test conventions",
                detail: `Found \`${depName}\` in dependencies. Once you add \`__tests__/\` directories or \`*.test.ts\` files, fubbik can auto-generate a testing conventions chunk.`,
            };
        },
        buildTipFromFiles(files) {
            return {
                title: "Consider adding a test runner",
                detail: `Found test files (${files.slice(0, 2).join(", ")}${files.length > 2 ? "…" : ""}) but no recognized test runner in package.json. Consider adding vitest or jest.`,
            };
        },
    },

    // 3. Database / ORM
    {
        name: "database",
        deps: [
            "drizzle-orm",
            "prisma",
            "@prisma/client",
            "mongoose",
            "sequelize",
            "typeorm",
            "mikro-orm",
            "@mikro-orm/core",
            "knex",
        ],
        findFiles(dir) {
            const found: string[] = [];
            // schema.ts / schema.js files anywhere in the tree
            const schemaFiles = findFilesByPattern(dir, /^schema\.(ts|js)$/);
            found.push(...schemaFiles);
            // migrations/ or db/ or database/ directories
            searchDirs(dir, (path, isDir, name) => {
                if (isDir && (name === "migrations" || name === "db" || name === "database")) {
                    found.push(path);
                }
            });
            return { found: found.length > 0, paths: found };
        },
        buildChunk(_dir, depName, version, files) {
            return {
                title: "Database & ORM Conventions",
                content: [
                    `This project uses **${depName}** (${version}) for database access.`,
                    "",
                    "## Database Files",
                    ...files.map(f => `- \`${f}\``),
                    "",
                    "Define schema changes in migrations and keep schema definitions co-located with database logic.",
                ].join("\n"),
                type: "note",
                tags: ["database", "conventions", depName],
                tier: 3,
                category: "conventions",
                source: `detected:database:${depName}`,
            };
        },
        buildTipFromDep(depName) {
            return {
                title: "Consider documenting your database conventions",
                detail: `Found \`${depName}\` in dependencies. Once you add schema files or a \`migrations/\` directory, fubbik can auto-generate a database conventions chunk.`,
            };
        },
        buildTipFromFiles(files) {
            return {
                title: "Consider adding a database ORM",
                detail: `Found database-related files (${files.slice(0, 2).join(", ")}${files.length > 2 ? "…" : ""}) but no recognized ORM in package.json.`,
            };
        },
    },

    // 4. Component structure
    {
        name: "components",
        deps: ["react", "vue", "svelte", "@angular/core", "solid-js", "preact", "qwik"],
        findFiles(dir) {
            const found: string[] = [];
            const names = ["components", "features", "ui"];
            for (const name of names) {
                const srcPath = join(dir, "src", name);
                const rootPath = join(dir, name);
                if (dirExists(srcPath)) found.push(srcPath);
                else if (dirExists(rootPath)) found.push(rootPath);
            }
            if (found.length === 0) {
                searchDirs(dir, (path, isDir, name) => {
                    if (isDir && (name === "components" || name === "features" || name === "ui")) {
                        found.push(path);
                    }
                });
            }
            return { found: found.length > 0, paths: found };
        },
        buildChunk(_dir, depName, version, files) {
            return {
                title: "Component Structure Conventions",
                content: [
                    `This project uses **${depName}** (${version}) for UI components.`,
                    "",
                    "## Component Directories",
                    ...files.map(f => `- \`${f}\``),
                    "",
                    "Organize components by feature or domain. Co-locate styles, tests, and types with each component.",
                ].join("\n"),
                type: "note",
                tags: ["components", "structure", depName],
                tier: 3,
                category: "structure",
                source: `detected:components:${depName}`,
                appliesTo: files.map(f => `${f}/**`),
            };
        },
        buildTipFromDep(depName) {
            return {
                title: "Consider documenting your component structure",
                detail: `Found \`${depName}\` in dependencies. Once you add a \`components/\`, \`features/\`, or \`ui/\` directory, fubbik can auto-generate a component structure chunk.`,
            };
        },
        buildTipFromFiles(files) {
            return {
                title: "Consider adding a UI framework",
                detail: `Found component directories (${files.join(", ")}) but no recognized UI framework in package.json.`,
            };
        },
    },

    // 5. Auth
    {
        name: "auth",
        deps: [
            "better-auth",
            "next-auth",
            "passport",
            "@auth/core",
            "lucia",
            "oslo",
            "clerk",
            "@clerk/nextjs",
            "supabase",
        ],
        findFiles(dir) {
            const found: string[] = [];
            // auth.ts / auth.js
            const authFiles = findFilesByPattern(dir, /^auth\.(ts|js)$/);
            found.push(...authFiles);
            // auth/ directories
            searchDirs(dir, (path, isDir, name) => {
                if (isDir && name === "auth") {
                    found.push(path);
                }
            });
            return { found: found.length > 0, paths: found };
        },
        buildChunk(_dir, depName, version, files) {
            return {
                title: "Authentication Conventions",
                content: [
                    `This project uses **${depName}** (${version}) for authentication.`,
                    "",
                    "## Auth Files",
                    ...files.map(f => `- \`${f}\``),
                    "",
                    "Document your session strategy, protected route patterns, and token handling approach.",
                ].join("\n"),
                type: "note",
                tags: ["auth", "conventions", depName],
                tier: 3,
                category: "conventions",
                source: `detected:auth:${depName}`,
            };
        },
        buildTipFromDep(depName) {
            return {
                title: "Consider documenting your auth conventions",
                detail: `Found \`${depName}\` in dependencies. Once you add \`auth.ts\` or an \`auth/\` directory, fubbik can auto-generate an authentication conventions chunk.`,
            };
        },
        buildTipFromFiles(files) {
            return {
                title: "Consider adding an auth library",
                detail: `Found auth-related files (${files.slice(0, 2).join(", ")}${files.length > 2 ? "…" : ""}) but no recognized auth library in package.json.`,
            };
        },
    },
];

// --- Main export ---

export function scanPatterns(dir: string): PatternResult {
    const chunks: DiscoveredChunk[] = [];
    const tips: Tip[] = [];

    const pkg = readPackageJson(dir);

    for (const detector of DETECTORS) {
        const depMatch = findDep(pkg, detector.deps);
        const fileResult = detector.findFiles(dir);

        if (depMatch && fileResult.found) {
            // Both signals → emit a chunk
            chunks.push(
                detector.buildChunk(dir, depMatch.name, depMatch.version, fileResult.paths),
            );
        } else if (depMatch && !fileResult.found) {
            // Dep only → emit a tip
            tips.push(detector.buildTipFromDep(depMatch.name));
        } else if (!depMatch && fileResult.found) {
            // Files only → emit a tip
            tips.push(detector.buildTipFromFiles(fileResult.paths));
        }
        // Neither → skip silently
    }

    return { chunks, tips };
}
