import { execSync } from "node:child_process";

import { Command } from "commander";

import { formatBold, formatDim, formatType } from "../lib/colors";
import { globMatch } from "../lib/glob-match";
import { isJson } from "../lib/output";
import { getServerUrl } from "../lib/store";

interface MatchedChunk {
    id: string;
    title: string;
    type?: string;
    matchedFiles: string[];
    matchReason: "file-ref" | "applies-to";
}

async function getStagedFiles(): Promise<string[]> {
    const out = execSync("git diff --cached --name-only", { encoding: "utf-8" });
    return out
        .split("\n")
        .map(f => f.trim())
        .filter(Boolean);
}

async function lookupFileRefs(
    serverUrl: string,
    file: string
): Promise<{ id: string; title: string; type: string }[]> {
    try {
        const res = await fetch(
            `${serverUrl}/api/file-refs/lookup?path=${encodeURIComponent(file)}`
        );
        if (!res.ok) return [];
        return (await res.json()) as { id: string; title: string; type: string }[];
    } catch {
        return [];
    }
}

async function getChunkAppliesTo(
    serverUrl: string,
    chunkId: string
): Promise<string[]> {
    try {
        const res = await fetch(`${serverUrl}/api/chunks/${chunkId}/applies-to`);
        if (!res.ok) return [];
        const data = (await res.json()) as { patterns: string[] } | string[];
        if (Array.isArray(data)) return data;
        if (data && Array.isArray(data.patterns)) return data.patterns;
        return [];
    } catch {
        return [];
    }
}

async function getAllChunks(
    serverUrl: string
): Promise<{ id: string; title: string; type: string }[]> {
    try {
        const res = await fetch(`${serverUrl}/api/chunks?limit=1000`);
        if (!res.ok) return [];
        const data = (await res.json()) as
            | { id: string; title: string; type: string }[]
            | { data: { id: string; title: string; type: string }[] };
        if (Array.isArray(data)) return data;
        if (data && "data" in data && Array.isArray(data.data)) return data.data;
        return [];
    } catch {
        return [];
    }
}

export const checkFilesCommand = new Command("check-files")
    .description("Check if files are associated with any knowledge chunks")
    .argument("[files...]", "files to check")
    .option("--staged", "check git staged files")
    .option("--json", "output as JSON")
    .action(
        async (
            files: string[],
            opts: { staged?: boolean; json?: boolean },
            cmd: Command
        ) => {
            let serverUrl: string | undefined;
            try {
                serverUrl = getServerUrl();
            } catch {
                // No store or no server URL — silently exit (important for hook context)
                return;
            }

            if (!serverUrl) return;

            const filesToCheck =
                opts.staged || files.length === 0
                    ? await getStagedFiles()
                    : files;

            if (filesToCheck.length === 0) return;

            const matched = new Map<string, MatchedChunk>();

            // 1. Check file-refs for each file
            for (const file of filesToCheck) {
                const refs = await lookupFileRefs(serverUrl, file);
                for (const ref of refs) {
                    const existing = matched.get(ref.id);
                    if (existing) {
                        if (!existing.matchedFiles.includes(file)) {
                            existing.matchedFiles.push(file);
                        }
                    } else {
                        matched.set(ref.id, {
                            id: ref.id,
                            title: ref.title,
                            type: ref.type,
                            matchedFiles: [file],
                            matchReason: "file-ref",
                        });
                    }
                }
            }

            // 2. Check applies-to glob patterns
            const allChunks = await getAllChunks(serverUrl);
            for (const chunk of allChunks) {
                if (matched.has(chunk.id)) continue;
                const patterns = await getChunkAppliesTo(serverUrl, chunk.id);
                if (patterns.length === 0) continue;

                const matchingFiles: string[] = [];
                for (const file of filesToCheck) {
                    for (const pattern of patterns) {
                        if (globMatch(pattern, file)) {
                            matchingFiles.push(file);
                            break;
                        }
                    }
                }

                if (matchingFiles.length > 0) {
                    matched.set(chunk.id, {
                        id: chunk.id,
                        title: chunk.title,
                        type: chunk.type,
                        matchedFiles: matchingFiles,
                        matchReason: "applies-to",
                    });
                }
            }

            if (matched.size === 0) return;

            const results = Array.from(matched.values());

            if (opts.json || isJson(cmd)) {
                console.log(JSON.stringify(results, null, 2));
                return;
            }

            // Print grouped warnings to stderr
            console.error(
                formatBold(
                    `\n  ${matched.size} chunk(s) related to your changes:\n`
                )
            );
            for (const chunk of results) {
                const typeStr = chunk.type ? ` ${formatType(chunk.type)}` : "";
                console.error(`  ${formatBold(chunk.title)}${typeStr}`);
                for (const file of chunk.matchedFiles) {
                    console.error(`    ${formatDim(file)}`);
                }
                console.error("");
            }
        }
    );
