import { useCallback, useRef, useState } from "react";
import { env } from "@fubbik/env/web";
import type { ImportFileStatus } from "./types";

interface SSEImportOptions {
    files: { path: string; content: string }[];
    codebaseId: string;
    templateOverrides?: Record<string, string | null>;
    onFileUpdate: (path: string, status: ImportFileStatus) => void;
    onDone: (result: { created: number; skipped: number; errors: number; connections: number; elapsed: number }) => void;
    onError: (error: string) => void;
}

export function useSSEImport() {
    const [isImporting, setIsImporting] = useState(false);
    const abortRef = useRef<AbortController | null>(null);

    const startImport = useCallback(async (options: SSEImportOptions) => {
        setIsImporting(true);
        const abort = new AbortController();
        abortRef.current = abort;

        try {
            const response = await fetch(`${env.VITE_SERVER_URL}/api/chunks/import-docs/stream`, {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                credentials: "include",
                signal: abort.signal,
                body: JSON.stringify({
                    files: options.files,
                    codebaseId: options.codebaseId,
                    templateOverrides: options.templateOverrides,
                }),
            });

            if (!response.ok || !response.body) {
                throw new Error(`HTTP ${response.status}`);
            }

            const reader = response.body.getReader();
            const decoder = new TextDecoder();
            let buffer = "";

            while (true) {
                const { done, value } = await reader.read();
                if (done) break;

                buffer += decoder.decode(value, { stream: true });
                const lines = buffer.split("\n");
                buffer = lines.pop() ?? "";

                let currentEvent = "";
                for (const line of lines) {
                    if (line.startsWith("event: ")) {
                        currentEvent = line.slice(7);
                    } else if (line.startsWith("data: ")) {
                        const data = JSON.parse(line.slice(6));
                        if (currentEvent === "file") {
                            options.onFileUpdate(data.path, {
                                status: data.status === "unchanged" ? "skipped" : data.status,
                                created: data.created,
                                error: data.error,
                            });
                        } else if (currentEvent === "done") {
                            options.onDone(data);
                        }
                        currentEvent = "";
                    }
                }
            }
        } catch (err) {
            if (!abort.signal.aborted) {
                options.onError(String(err));
            }
        } finally {
            setIsImporting(false);
        }
    }, []);

    return { startImport, isImporting };
}
