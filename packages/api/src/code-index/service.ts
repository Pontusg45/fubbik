import { readdir, readFile } from "node:fs/promises";
import { extname, join, relative } from "node:path";

import { cypherVoid, escCypher } from "@fubbik/db/age/client";
import { createEdge, deleteEdgesFrom, ensureVertex } from "@fubbik/db/age/sync";
import { isAgeAvailable, listAllFileRefs } from "@fubbik/db/repository";
// packages/api/src/code-index/service.ts
import { Effect } from "effect";

import { logger } from "../logger";

export interface ExtractedSymbol {
    name: string;
    kind: "function" | "class" | "type" | "interface" | "variable";
    line: number;
    exported: boolean;
}

export interface IndexedFile {
    path: string;
    language: string;
    symbols: ExtractedSymbol[];
    imports: string[];
}

const SUPPORTED_EXTENSIONS: Record<string, string> = {
    ".ts": "typescript",
    ".tsx": "typescript",
    ".js": "javascript",
    ".jsx": "javascript"
};

function extractSymbolsRegex(content: string): ExtractedSymbol[] {
    const symbols: ExtractedSymbol[] = [];
    const lines = content.split("\n");

    const exportPatterns: { regex: RegExp; kind: ExtractedSymbol["kind"] }[] = [
        { regex: /^export\s+(?:async\s+)?function\s+(\w+)/, kind: "function" },
        { regex: /^export\s+class\s+(\w+)/, kind: "class" },
        { regex: /^export\s+type\s+(\w+)/, kind: "type" },
        { regex: /^export\s+interface\s+(\w+)/, kind: "interface" },
        { regex: /^export\s+const\s+(\w+)/, kind: "variable" },
        { regex: /^export\s+default\s+(?:async\s+)?function\s+(\w+)/, kind: "function" },
        { regex: /^export\s+default\s+class\s+(\w+)/, kind: "class" }
    ];

    for (let i = 0; i < lines.length; i++) {
        const line = lines[i]!.trim();
        for (const { regex, kind } of exportPatterns) {
            const m = line.match(regex);
            if (m && m[1]) {
                symbols.push({ name: m[1], kind, line: i + 1, exported: true });
                break;
            }
        }
    }

    return symbols;
}

function extractImports(content: string): string[] {
    const imports: string[] = [];
    const importRegex = /(?:import|from)\s+['"]([^'"]+)['"]/g;
    let match;
    while ((match = importRegex.exec(content)) !== null) {
        if (match[1] && match[1].startsWith(".")) {
            imports.push(match[1]);
        }
    }
    return imports;
}

export function indexFile(filePath: string, basePath: string): Effect.Effect<IndexedFile | null, Error> {
    return Effect.tryPromise({
        try: async () => {
            const ext = extname(filePath);
            const language = SUPPORTED_EXTENSIONS[ext];
            if (!language) return null;

            const content = await readFile(filePath, "utf-8");
            const relPath = relative(basePath, filePath);
            const symbols = extractSymbolsRegex(content);
            const imports = extractImports(content);

            return { path: relPath, language, symbols, imports } satisfies IndexedFile;
        },
        catch: cause => new Error(`Failed to index ${filePath}: ${cause}`)
    });
}

export function indexDirectory(dirPath: string, basePath?: string): Effect.Effect<IndexedFile[], Error> {
    const base = basePath ?? dirPath;
    return Effect.tryPromise({
        try: async () => {
            const files: IndexedFile[] = [];

            async function walk(dir: string) {
                const entries = await readdir(dir, { withFileTypes: true });
                for (const entry of entries) {
                    const full = join(dir, entry.name);
                    if (entry.name.startsWith(".") || entry.name === "node_modules" || entry.name === "dist") continue;
                    if (entry.isDirectory()) {
                        await walk(full);
                    } else if (SUPPORTED_EXTENSIONS[extname(entry.name)]) {
                        const result = await Effect.runPromise(indexFile(full, base));
                        if (result) files.push(result);
                    }
                }
            }

            await walk(dirPath);
            return files;
        },
        catch: cause => new Error(`Failed to index directory ${dirPath}: ${cause}`)
    });
}

export function syncIndexToGraph(files: IndexedFile[]) {
    return Effect.gen(function* () {
        const available = yield* Effect.promise(() => isAgeAvailable());
        if (!available) return { synced: 0 };

        const now = new Date().toISOString();
        let synced = 0;

        for (const file of files) {
            yield* ensureVertex("code_file", file.path);
            yield* cypherVoid(
                `MATCH (f:code_file {id: '${escCypher(file.path)}'})
                 SET f.language = '${escCypher(file.language)}', f.lastIndexedAt = '${escCypher(now)}'`
            );

            yield* deleteEdgesFrom("defines", "code_file", file.path);

            for (const sym of file.symbols.filter(s => s.exported)) {
                const symId = `${file.path}::${sym.name}`;
                yield* ensureVertex("code_symbol", symId);
                yield* cypherVoid(
                    `MATCH (s:code_symbol {id: '${escCypher(symId)}'})
                     SET s.name = '${escCypher(sym.name)}', s.kind = '${escCypher(sym.kind)}',
                         s.filePath = '${escCypher(file.path)}', s.line = ${sym.line},
                         s.exported = true`
                );
                yield* createEdge("defines", "code_file", file.path, "code_symbol", symId);
            }

            synced++;
        }

        logger.info("Code index synced to graph", { files: synced });
        return { synced };
    });
}

export function syncAnnotatesEdges(userId: string) {
    return Effect.gen(function* () {
        const ageReady = yield* Effect.promise(() => isAgeAvailable());
        if (!ageReady) return { linked: 0 };

        const fileRefs = yield* listAllFileRefs(userId);
        let linked = 0;

        for (const ref of fileRefs) {
            yield* cypherVoid(
                `MATCH (c:chunk {id: '${escCypher(ref.chunkId)}'}), (f:code_file)
                 WHERE f.id ENDS WITH '${escCypher(ref.path)}'
                 MERGE (c)-[:annotates {via: 'file_ref'}]->(f)`
            );
            linked++;
        }

        logger.info("Annotates edges synced", { linked });
        return { linked };
    });
}
