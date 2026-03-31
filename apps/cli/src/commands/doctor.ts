import { Command } from "commander";
import pc from "picocolors";
import { formatError, formatSuccess } from "../lib/colors";
import { output } from "../lib/output";
import { getServerUrl, readStore, storeExists } from "../lib/store";
import { detectCodebase } from "../lib/detect-codebase";

const warn = (msg: string) => `${pc.yellow("\u26A0")} ${msg}`;

export const doctorCommand = new Command("doctor")
    .description("Run diagnostics on the fubbik setup")
    .action(async (_opts: unknown, cmd: Command) => {
        const lines: string[] = [];
        const data: Record<string, unknown> = {};

        lines.push(pc.bold("Fubbik Doctor"));

        // 1. Local store
        if (storeExists()) {
            try {
                const store = readStore();
                const count = store.chunks.length;
                lines.push(`  ${formatSuccess(`Local store initialized (${count} chunks)`)}`);
                data.localStore = { ok: true, chunks: count };
            } catch {
                lines.push(`  ${formatError("Local store corrupted")}`);
                data.localStore = { ok: false };
            }
        } else {
            lines.push(`  ${formatError("Local store not found (run 'fubbik init')")}`);
            data.localStore = { ok: false };
        }

        // 2. Server connection
        const serverUrl = getServerUrl();
        if (!serverUrl) {
            lines.push(`  ${formatError("Server not configured")}`);
            data.server = { ok: false };
        } else {
            try {
                const healthRes = await fetch(`${serverUrl}/api/health`);
                if (healthRes.ok) {
                    lines.push(`  ${formatSuccess(`Server connected (${serverUrl})`)}`);
                    data.server = { ok: true, url: serverUrl };
                } else {
                    lines.push(`  ${formatError(`Server unhealthy (${serverUrl}, status ${healthRes.status})`)}`);
                    data.server = { ok: false, url: serverUrl, status: healthRes.status };
                }
            } catch {
                lines.push(`  ${formatError(`Server unreachable (${serverUrl})`)}`);
                data.server = { ok: false, url: serverUrl };
            }
        }

        // 3. Ollama
        const ollamaUrl = process.env.OLLAMA_URL ?? "http://localhost:11434";
        try {
            const ollamaRes = await fetch(`${ollamaUrl}/api/tags`, {
                signal: AbortSignal.timeout(3000)
            });
            if (ollamaRes.ok) {
                lines.push(`  ${formatSuccess(`Ollama reachable (${ollamaUrl})`)}`);
                data.ollama = { ok: true, url: ollamaUrl };
            } else {
                lines.push(`  ${formatError(`Ollama not reachable (${ollamaUrl})`)}`);
                data.ollama = { ok: false, url: ollamaUrl };
            }
        } catch {
            lines.push(`  ${formatError(`Ollama not reachable (${ollamaUrl})`)}`);
            data.ollama = { ok: false, url: ollamaUrl };
        }

        // 4. Codebase detected
        const codebase = await detectCodebase();
        if (codebase) {
            lines.push(`  ${formatSuccess(`Codebase detected (${codebase.name})`)}`);
            data.codebase = { ok: true, name: codebase.name, id: codebase.id };
        } else {
            lines.push(`  ${formatError("Codebase not detected")}`);
            data.codebase = { ok: false };
        }

        // 5. Stale embeddings
        if (serverUrl && data.server && (data.server as Record<string, unknown>).ok) {
            try {
                const knowledgeRes = await fetch(`${serverUrl}/api/health/knowledge`);
                if (knowledgeRes.ok) {
                    const health = (await knowledgeRes.json()) as {
                        staleEmbeddings?: number;
                        staleEmbeddingCount?: number;
                    };
                    const stale = health.staleEmbeddings ?? health.staleEmbeddingCount ?? 0;
                    if (stale > 0) {
                        lines.push(`  ${warn(`${stale} chunks missing embeddings`)}`);
                        data.staleEmbeddings = stale;
                    } else {
                        lines.push(`  ${formatSuccess("All embeddings up to date")}`);
                        data.staleEmbeddings = 0;
                    }
                }
            } catch {
                // skip if health/knowledge endpoint fails
            }
        }

        output(cmd, data, lines.join("\n"));
    });
