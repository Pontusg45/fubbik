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

export async function detectSpace(): Promise<{ id: string; name: string } | null> {
    const serverUrl = getServerUrl();
    if (!serverUrl) return null;

    const remoteUrl = getGitRemoteUrl();
    const localPath = process.cwd();

    const params = new URLSearchParams();
    if (remoteUrl) params.set("remoteUrl", remoteUrl);
    else params.set("localPath", localPath);

    try {
        const res = await fetch(`${serverUrl}/api/spaces/detect?${params}`);
        if (!res.ok) return null;
        const data = (await res.json()) as { id?: string; name?: string };
        return data && data.id ? { id: data.id, name: data.name! } : null;
    } catch {
        return null;
    }
}

export async function resolveSpaceId(
    serverUrl: string,
    opts: { global?: boolean; space?: string; codebase?: string }
): Promise<string | null> {
    if (opts.global) return null;

    const spaceName = opts.space ?? opts.codebase;

    if (spaceName) {
        // Look up space by name
        try {
            const res = await fetch(`${serverUrl}/api/spaces`);
            if (!res.ok) return null;
            const data = (await res.json()) as { id: string; name: string }[];
            const match = data.find(c => c.name === spaceName);
            if (match) return match.id;
            console.error(`Space "${spaceName}" not found.`);
            process.exit(1);
        } catch {
            return null;
        }
    }

    // Auto-detect from git remote / cwd
    const detected = await detectSpace();
    return detected?.id ?? null;
}

// Legacy aliases — remove after one release
export const detectCodebase = detectSpace;
export const resolveCodebaseId = resolveSpaceId;
