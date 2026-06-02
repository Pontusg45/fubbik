import { existsSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";

export interface FubbikConfig {
    serverUrl?: string;
    /** space name for auto-scoping (preferred) */
    space?: string;
    /** @deprecated use `space` instead */
    codebase?: string;
    defaultType?: string;
    claudeMd?: {
        tag?: string;
        output?: string;
    };
    hooks?: {
        preCommit?: boolean;
    };
    context?: {
        maxTokens?: number;
        includeDeps?: boolean;
    };
}

const CONFIG_FILES = ["fubbik.config.ts", "fubbik.config.json", ".fubbikrc.json"];

let cachedConfig: FubbikConfig | null = null;
let cachedConfigPath: string | null = null;

export function findConfigFile(startDir?: string): string | null {
    let dir = startDir ?? process.cwd();

    for (let i = 0; i < 20; i++) {
        for (const name of CONFIG_FILES) {
            const candidate = join(dir, name);
            if (existsSync(candidate)) return candidate;
        }
        // Stop at git root
        if (existsSync(join(dir, ".git"))) break;
        const parent = dirname(dir);
        if (parent === dir) break;
        dir = parent;
    }
    return null;
}

export function loadConfig(startDir?: string): FubbikConfig {
    if (cachedConfig) return cachedConfig;

    const configPath = findConfigFile(startDir);
    if (!configPath) {
        cachedConfig = {};
        return cachedConfig;
    }

    cachedConfigPath = configPath;

    try {
        if (configPath.endsWith(".ts")) {
            // Bun can import .ts natively — use require for sync loading
            // eslint-disable-next-line @typescript-eslint/no-require-imports
            const mod = require(configPath);
            cachedConfig = mod.default ?? mod;
        } else {
            const raw = readFileSync(configPath, "utf-8");
            cachedConfig = JSON.parse(raw);
        }
    } catch (e) {
        console.error(`Warning: Failed to parse ${configPath}:`, e);
        cachedConfig = {};
    }

    return cachedConfig!;
}

export function getConfigPath(): string | null {
    if (cachedConfigPath !== null) return cachedConfigPath;
    findConfigFile();
    return cachedConfigPath;
}

export async function resolveSpaceFromConfig(serverUrl: string): Promise<string | null> {
    const config = loadConfig();
    const spaceName = config.space ?? config.codebase;
    if (!spaceName) return null;

    try {
        const res = await fetch(`${serverUrl}/api/spaces`);
        if (!res.ok) return null;
        const spaces = (await res.json()) as { id: string; name: string }[];
        const match = spaces.find(
            (c) => c.name.toLowerCase() === spaceName.toLowerCase()
        );
        return match?.id ?? null;
    } catch {
        return null;
    }
}

// Legacy alias — remove after one release
export const resolveCodebaseFromConfig = resolveSpaceFromConfig;
