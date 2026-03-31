import { execSync } from "node:child_process";

import { getServerUrl } from "./store";

export function getGitRemoteUrl(): string | null {
    try {
        return (
            execSync("git remote get-url origin", {
                encoding: "utf-8",
                stdio: ["pipe", "pipe", "pipe"],
                cwd: process.cwd()
            }).trim() || null
        );
    } catch {
        return null;
    }
}

export async function detectCodebase(): Promise<{ id: string; name: string } | null> {
    const serverUrl = getServerUrl();
    if (!serverUrl) return null;

    const remoteUrl = getGitRemoteUrl();
    const localPath = process.cwd();

    const params = new URLSearchParams();
    if (remoteUrl) params.set("remoteUrl", remoteUrl);
    else params.set("localPath", localPath);

    try {
        const res = await fetch(`${serverUrl}/api/codebases/detect?${params}`);
        if (!res.ok) return null;
        const data = (await res.json()) as { id?: string; name?: string };
        return data && data.id ? { id: data.id, name: data.name! } : null;
    } catch {
        return null;
    }
}

export async function resolveCodebaseId(
    serverUrl: string,
    opts: { global?: boolean; codebase?: string }
): Promise<string | null> {
    if (opts.global) return null;

    if (opts.codebase) {
        // Look up codebase by name
        try {
            const res = await fetch(`${serverUrl}/api/codebases`);
            if (!res.ok) return null;
            const data = (await res.json()) as { id: string; name: string }[];
            const match = data.find(c => c.name === opts.codebase);
            if (match) return match.id;
            console.error(`Codebase "${opts.codebase}" not found.`);
            process.exit(1);
        } catch {
            return null;
        }
    }

    // Auto-detect from git remote / cwd
    const detected = await detectCodebase();
    return detected?.id ?? null;
}
