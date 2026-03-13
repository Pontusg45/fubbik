import * as vscode from "vscode";
import type { FubbikApi } from "./api";
import { getBaseHtml, getNonce } from "./webview-utils";

const CHUNK_TYPES = ["note", "document", "reference", "schema", "checklist"];

export function registerCreateChunkCommand(
    context: vscode.ExtensionContext,
    api: FubbikApi,
    getCodebaseId: () => string | null,
    onChunkCreated: () => void
): vscode.Disposable {
    return vscode.commands.registerCommand("fubbik.addChunk", () => {
        const editor = vscode.window.activeTextEditor;
        const selection = editor?.selection;
        const selectedText =
            editor && selection && !selection.isEmpty
                ? editor.document.getText(selection)
                : "";

        const firstLine = selectedText.split("\n")[0]?.trim() || "";

        const panel = vscode.window.createWebviewPanel(
            "fubbik.createChunk",
            "Add to Fubbik",
            vscode.ViewColumn.One,
            { enableScripts: true }
        );

        const codebaseId = getCodebaseId();

        const nonce = getNonce();
        const body = buildFormBody(firstLine, selectedText, codebaseId);
        const script = buildFormScript();

        panel.webview.html = getBaseHtml(panel.webview, nonce, body, script);

        panel.webview.onDidReceiveMessage(
            async (message) => {
                switch (message.type) {
                    case "submit": {
                        try {
                            const body: Parameters<typeof api.createChunk>[0] =
                                {
                                    content: message.content,
                                    title: message.title || undefined,
                                    source: message.chunkType || undefined,
                                    tags: message.tags || undefined,
                                };

                            if (codebaseId) {
                                body.codebaseId = codebaseId;
                            }

                            await api.createChunk(body);
                            vscode.window.showInformationMessage(
                                "Chunk added to Fubbik!"
                            );
                            panel.dispose();
                            onChunkCreated();
                        } catch (err) {
                            const errorMessage =
                                err instanceof Error
                                    ? err.message
                                    : "Unknown error";
                            vscode.window.showErrorMessage(
                                `Failed to create chunk: ${errorMessage}`
                            );
                        }
                        break;
                    }
                    case "cancel": {
                        panel.dispose();
                        break;
                    }
                }
            },
            undefined,
            context.subscriptions
        );
    });
}

function buildFormBody(
    title: string,
    content: string,
    codebaseId: string | null
): string {
    let html = `<div style="max-width:600px;">`;
    html += `<h2 class="mb-3">Add to Fubbik</h2>`;

    if (!codebaseId) {
        html += `<p class="muted mb-3" style="font-size:0.9em;">No codebase detected &mdash; chunk will be global.</p>`;
    }

    // Title
    html += `<div class="mb-2">`;
    html += `<label class="mb-1" style="display:block;font-weight:600;">Title</label>`;
    html += `<input type="text" id="titleInput" value="${escapeAttr(title)}" />`;
    html += `</div>`;

    // Content
    html += `<div class="mb-2">`;
    html += `<label class="mb-1" style="display:block;font-weight:600;">Content</label>`;
    html += `<textarea id="contentInput" rows="10">${escapeHtml(content)}</textarea>`;
    html += `</div>`;

    // Type
    html += `<div class="mb-2">`;
    html += `<label class="mb-1" style="display:block;font-weight:600;">Type</label>`;
    html += `<select id="typeSelect">`;
    for (const t of CHUNK_TYPES) {
        html += `<option value="${t}">${t}</option>`;
    }
    html += `</select>`;
    html += `</div>`;

    // Tags
    html += `<div class="mb-3">`;
    html += `<label class="mb-1" style="display:block;font-weight:600;">Tags (comma-separated)</label>`;
    html += `<input type="text" id="tagsInput" placeholder="e.g. api, auth, database" />`;
    html += `</div>`;

    // Buttons
    html += `<div style="display:flex;gap:8px;">`;
    html += `<button id="submitBtn">Submit</button>`;
    html += `<button id="cancelBtn" class="secondary">Cancel</button>`;
    html += `</div>`;

    html += `</div>`;
    return html;
}

function buildFormScript(): string {
    return `
        const vscode = acquireVsCodeApi();

        document.getElementById("submitBtn")?.addEventListener("click", () => {
            const title = document.getElementById("titleInput").value.trim();
            const content = document.getElementById("contentInput").value.trim();
            const chunkType = document.getElementById("typeSelect").value;
            const tagsRaw = document.getElementById("tagsInput").value.trim();
            const tags = tagsRaw
                ? tagsRaw.split(",").map((t) => t.trim()).filter(Boolean)
                : undefined;

            if (!content) {
                return;
            }

            vscode.postMessage({
                type: "submit",
                title,
                content,
                chunkType,
                tags,
            });
        });

        document.getElementById("cancelBtn")?.addEventListener("click", () => {
            vscode.postMessage({ type: "cancel" });
        });
    `;
}

function escapeHtml(text: string): string {
    return text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

function escapeAttr(text: string): string {
    return text
        .replace(/&/g, "&amp;")
        .replace(/"/g, "&quot;")
        .replace(/</g, "&lt;")
        .replace(/>/g, "&gt;");
}
