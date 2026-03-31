import { Command } from "commander";
import { formatDim, formatError, formatSuccess } from "../lib/colors";
import { output } from "../lib/output";
import { getServerUrl, readStore, storeExists } from "../lib/store";
import { detectCodebase } from "../lib/detect-codebase";

export const statusCommand = new Command("status")
    .description("Show knowledge base status overview")
    .action(async (_opts: unknown, cmd: Command) => {
        const lines: string[] = [];
        const data: Record<string, unknown> = {};

        if (!storeExists()) {
            lines.push(formatError("No local store. Run 'fubbik init' to get started."));
            output(cmd, { initialized: false }, lines.join("\n"));
            return;
        }

        const store = readStore();
        data.name = store.name;
        data.localChunks = store.chunks.length;
        data.lastSync = store.lastSync ?? null;

        lines.push(`Knowledge base: ${store.name}`);
        lines.push(`  Local chunks: ${store.chunks.length}`);
        lines.push(`  Last sync: ${store.lastSync ? new Date(store.lastSync).toLocaleString() : formatDim("never")}`);

        const serverUrl = getServerUrl();
        if (!serverUrl) {
            lines.push(`  Server: ${formatDim("not configured")}`);
        } else {
            lines.push(`  Server: ${serverUrl}`);
            try {
                const healthRes = await fetch(`${serverUrl}/api/health`);
                if (healthRes.ok) {
                    lines.push(`  Connection: ${formatSuccess("connected")}`);
                    data.serverConnected = true;
                } else {
                    lines.push(`  Connection: ${formatError("unhealthy")} (${healthRes.status})`);
                    data.serverConnected = false;
                }
            } catch {
                lines.push(`  Connection: ${formatError("unreachable")}`);
                data.serverConnected = false;
            }
        }

        const codebase = await detectCodebase();
        if (codebase) {
            lines.push(`  Codebase: ${codebase.name}`);
            data.codebase = codebase;
        } else {
            lines.push(`  Codebase: ${formatDim("not detected")}`);
        }

        output(cmd, data, lines.join("\n"));
    });
