import * as vscode from "vscode";
import type { Chunk, FubbikApi } from "./api";
import { getBaseHtml, getNonce } from "./webview-utils";

interface SidebarState {
    codebaseName?: string;
    chunks?: Chunk[];
    total?: number;
    error?: string;
    loading?: boolean;
}

export class SidebarProvider implements vscode.WebviewViewProvider {
    public static readonly viewType = "fubbik.sidebar";

    private view?: vscode.WebviewView;
    private state: SidebarState = {};

    private api?: FubbikApi;

    constructor(private readonly extensionUri: vscode.Uri) {}

    public setApi(api: FubbikApi) {
        this.api = api;
    }

    public resolveWebviewView(
        webviewView: vscode.WebviewView,
        _context: vscode.WebviewViewResolveContext,
        _token: vscode.CancellationToken
    ): void {
        this.view = webviewView;

        webviewView.webview.options = {
            enableScripts: true,
        };

        webviewView.webview.onDidReceiveMessage(async (message) => {
            switch (message.type) {
                case "openChunk": {
                    if (this.api) {
                        const { showChunkDetail } = await import("./chunk-detail");
                        showChunkDetail(this.api, message.id);
                    }
                    break;
                }
                case "refresh": {
                    vscode.commands.executeCommand("fubbik.refreshSidebar");
                    break;
                }
            }
        });

        this.render();
    }

    public setState(state: SidebarState): void {
        this.state = { ...this.state, ...state };
        this.render();
    }

    private render(): void {
        if (!this.view) {
            return;
        }

        const nonce = getNonce();
        const body = this.buildBody();
        const script = this.buildScript();

        this.view.webview.html = getBaseHtml(
            this.view.webview,
            nonce,
            body,
            script
        );
    }

    private buildBody(): string {
        const { codebaseName, chunks, total, error, loading } = this.state;

        let html = `<div id="sidebar">`;

        // Header
        html += `<div style="display:flex;align-items:center;justify-content:space-between;" class="mb-3">`;
        html += `<h2>${codebaseName ? escapeHtml(codebaseName) : "Fubbik"}</h2>`;
        html += `<button id="refreshBtn" title="Refresh" style="padding:4px 8px;font-size:1.1em;">&#8635;</button>`;
        html += `</div>`;

        if (loading) {
            html += `<p class="muted">Loading chunks...</p>`;
            html += `</div>`;
            return html;
        }

        if (error) {
            html += `<p class="error">${escapeHtml(error)}</p>`;
            html += `</div>`;
            return html;
        }

        if (!chunks || chunks.length === 0) {
            html += `<p class="muted">No chunks found</p>`;
            html += `</div>`;
            return html;
        }

        // Search input
        html += `<input type="text" id="searchInput" placeholder="Search chunks..." class="mb-2" />`;

        // Info about truncation
        if (total && total > 100) {
            html += `<p class="muted mb-2" style="font-size:0.9em;">Showing first 100 of ${total} chunks</p>`;
        }

        // Chunk list
        html += `<div id="chunkList">`;
        for (const chunk of chunks) {
            const title = chunk.title || "Untitled";
            const date = new Date(chunk.createdAt).toLocaleDateString();
            const type = chunk.source || "note";
            html += `<div class="list-item chunk-item" data-id="${escapeAttr(chunk.id)}" data-title="${escapeAttr(title)}">`;
            html += `<span class="badge">${escapeHtml(type)}</span> `;
            html += `<a href="#" class="chunk-link">${escapeHtml(title)}</a>`;
            html += `<div class="muted" style="font-size:0.85em;margin-top:2px;">${date}</div>`;
            html += `</div>`;
        }
        html += `</div>`;

        html += `</div>`;
        return html;
    }

    private buildScript(): string {
        return `
            const vscode = acquireVsCodeApi();

            document.getElementById("refreshBtn")?.addEventListener("click", () => {
                vscode.postMessage({ type: "refresh" });
            });

            document.getElementById("searchInput")?.addEventListener("input", (e) => {
                const query = e.target.value.toLowerCase();
                const items = document.querySelectorAll(".chunk-item");
                items.forEach((item) => {
                    const title = item.getAttribute("data-title")?.toLowerCase() || "";
                    item.style.display = title.includes(query) ? "" : "none";
                });
            });

            document.getElementById("chunkList")?.addEventListener("click", (e) => {
                const item = e.target.closest(".chunk-item");
                if (item) {
                    const id = item.getAttribute("data-id");
                    if (id) {
                        vscode.postMessage({ type: "openChunk", id });
                    }
                }
            });
        `;
    }
}

function escapeHtml(text: string): string {
    return text
        .replace(/&/g, "&amp;")
        .replace(/</g, "&lt;")
        .replace(/>/g, "&gt;")
        .replace(/"/g, "&quot;");
}

function escapeAttr(text: string): string {
    return text
        .replace(/&/g, "&amp;")
        .replace(/"/g, "&quot;")
        .replace(/</g, "&lt;")
        .replace(/>/g, "&gt;");
}
