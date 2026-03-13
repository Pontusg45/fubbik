import * as vscode from "vscode";

import type { FubbikApi } from "./api";
import { getBaseHtml, getNonce } from "./webview-utils";

export function showChunkDetail(api: FubbikApi, chunkId: string) {
    const panel = vscode.window.createWebviewPanel(
        "fubbik.chunkDetail",
        "Loading...",
        vscode.ViewColumn.One,
        { enableScripts: true }
    );

    const nonce = getNonce();

    // Show loading state
    panel.webview.html = getBaseHtml(
        panel.webview,
        nonce,
        `<p class="muted">Loading chunk...</p>`,
        ""
    );

    // Fetch and render
    api.getChunk(chunkId).then(({ chunk }) => {
        panel.title = chunk.title || "Untitled Chunk";
        const freshNonce = getNonce();

        const date = new Date(chunk.createdAt).toLocaleDateString();
        const updated = new Date(chunk.updatedAt).toLocaleDateString();
        const type = chunk.source || "note";
        const tags = chunk.tags ?? [];

        let body = "";

        // Header
        body += `<div style="margin-bottom:16px;">`;
        body += `<h2 style="margin:0 0 4px 0;">${escapeHtml(chunk.title || "Untitled")}</h2>`;
        body += `<div style="display:flex;gap:8px;align-items:center;flex-wrap:wrap;">`;
        body += `<span class="badge">${escapeHtml(type)}</span>`;
        for (const tag of tags) {
            body += `<span class="badge" style="background:var(--vscode-badge-background);opacity:0.8;">${escapeHtml(tag)}</span>`;
        }
        body += `<span class="muted" style="font-size:0.85em;">Created ${date}</span>`;
        if (updated !== date) {
            body += `<span class="muted" style="font-size:0.85em;">· Updated ${updated}</span>`;
        }
        body += `</div>`;
        body += `</div>`;

        // Content
        body += `<div style="white-space:pre-wrap;font-family:var(--vscode-editor-font-family,monospace);font-size:var(--vscode-editor-font-size,13px);line-height:1.6;background:var(--vscode-editor-background);border:1px solid var(--vscode-input-border);border-radius:4px;padding:12px;overflow-x:auto;">`;
        body += escapeHtml(chunk.content || "(empty)");
        body += `</div>`;

        // Actions
        body += `<div style="margin-top:12px;display:flex;gap:8px;">`;
        body += `<button onclick="openInBrowser()">Open in Browser</button>`;
        body += `<button class="secondary" onclick="copyContent()">Copy Content</button>`;
        body += `</div>`;

        const script = `
            const vscode = acquireVsCodeApi();
            function openInBrowser() {
                vscode.postMessage({ type: "openInBrowser" });
            }
            function copyContent() {
                vscode.postMessage({ type: "copyContent" });
            }
        `;

        panel.webview.html = getBaseHtml(panel.webview, freshNonce, body, script);

        panel.webview.onDidReceiveMessage(msg => {
            if (msg.type === "openInBrowser") {
                const webAppUrl = vscode.workspace
                    .getConfiguration("fubbik")
                    .get<string>("webAppUrl", "http://localhost:3001");
                vscode.env.openExternal(vscode.Uri.parse(`${webAppUrl}/chunks/${chunkId}`));
            } else if (msg.type === "copyContent") {
                vscode.env.clipboard.writeText(chunk.content || "");
                vscode.window.showInformationMessage("Content copied to clipboard");
            }
        });
    }).catch(err => {
        const freshNonce = getNonce();
        panel.webview.html = getBaseHtml(
            panel.webview,
            freshNonce,
            `<p class="error">Failed to load chunk: ${escapeHtml(err instanceof Error ? err.message : String(err))}</p>`,
            ""
        );
    });
}

function escapeHtml(text: string): string {
    return text
        .replace(/&/g, "&amp;")
        .replace(/</g, "&lt;")
        .replace(/>/g, "&gt;")
        .replace(/"/g, "&quot;");
}
