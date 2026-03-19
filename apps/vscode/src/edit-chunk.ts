import * as vscode from "vscode";
import type { Chunk, FubbikApi } from "./api";
import { getBaseHtml, getNonce } from "./webview-utils";

const CHUNK_TYPES = ["note", "document", "reference", "schema", "checklist"];

export function showEditChunk(
    api: FubbikApi,
    chunk: Chunk,
    onUpdated: () => void
) {
    const panel = vscode.window.createWebviewPanel(
        "fubbik.editChunk",
        `Edit: ${chunk.title || "Untitled"}`,
        vscode.ViewColumn.One,
        { enableScripts: true }
    );

    const nonce = getNonce();
    const body = buildEditForm(chunk);
    const script = buildEditScript();

    panel.webview.html = getBaseHtml(panel.webview, nonce, body, script);

    panel.webview.onDidReceiveMessage(async (msg) => {
        if (msg.type === "submit") {
            try {
                await api.updateChunk(chunk.id, msg.data);
                vscode.window.showInformationMessage("Chunk updated");
                panel.dispose();
                onUpdated();
            } catch (err: unknown) {
                const message =
                    err instanceof Error ? err.message : "Unknown error";
                vscode.window.showErrorMessage(
                    `Failed to update chunk: ${message}`
                );
            }
        }
        if (msg.type === "cancel") {
            panel.dispose();
        }
    });
}

function buildEditForm(chunk: Chunk): string {
    const currentType = chunk.source || "note";
    const currentTags = (chunk.tags || []).join(", ");

    let html = `<div style="max-width:600px;">`;
    html += `<h2 class="mb-3">Edit Chunk</h2>`;

    // Title
    html += `<div class="mb-2">`;
    html += `<label class="mb-1" style="display:block;font-weight:600;">Title</label>`;
    html += `<input type="text" id="titleInput" value="${escapeAttr(chunk.title || "")}" />`;
    html += `</div>`;

    // Content
    html += `<div class="mb-2">`;
    html += `<label class="mb-1" style="display:block;font-weight:600;">Content</label>`;
    html += `<textarea id="contentInput" rows="10">${escapeHtml(chunk.content || "")}</textarea>`;
    html += `</div>`;

    // Type
    html += `<div class="mb-2">`;
    html += `<label class="mb-1" style="display:block;font-weight:600;">Type</label>`;
    html += `<select id="typeSelect">`;
    for (const t of CHUNK_TYPES) {
        const selected = t === currentType ? " selected" : "";
        html += `<option value="${t}"${selected}>${t}</option>`;
    }
    html += `</select>`;
    html += `</div>`;

    // Tags
    html += `<div class="mb-3">`;
    html += `<label class="mb-1" style="display:block;font-weight:600;">Tags (comma-separated)</label>`;
    html += `<input type="text" id="tagsInput" value="${escapeAttr(currentTags)}" placeholder="e.g. api, auth, database" />`;
    html += `</div>`;

    // Buttons
    html += `<div style="display:flex;gap:8px;">`;
    html += `<button id="submitBtn">Save Changes</button>`;
    html += `<button id="cancelBtn" class="secondary">Cancel</button>`;
    html += `</div>`;

    html += `</div>`;
    return html;
}

function buildEditScript(): string {
    return `
        const vscode = acquireVsCodeApi();

        document.getElementById("submitBtn")?.addEventListener("click", () => {
            const title = document.getElementById("titleInput").value.trim();
            const content = document.getElementById("contentInput").value.trim();
            const source = document.getElementById("typeSelect").value;
            const tagsRaw = document.getElementById("tagsInput").value.trim();
            const tags = tagsRaw
                ? tagsRaw.split(",").map((t) => t.trim()).filter(Boolean)
                : [];

            if (!content) {
                return;
            }

            vscode.postMessage({
                type: "submit",
                data: { title, content, source, tags },
            });
        });

        document.getElementById("cancelBtn")?.addEventListener("click", () => {
            vscode.postMessage({ type: "cancel" });
        });
    `;
}

function escapeHtml(text: string): string {
    return text
        .replace(/&/g, "&amp;")
        .replace(/</g, "&lt;")
        .replace(/>/g, "&gt;");
}

function escapeAttr(text: string): string {
    return text
        .replace(/&/g, "&amp;")
        .replace(/"/g, "&quot;")
        .replace(/</g, "&lt;")
        .replace(/>/g, "&gt;");
}
