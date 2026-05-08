/* ─── Document Browser Utilities ─── */

import type { DocumentListItem, DocumentChunk } from "./document-types";

/* ─── Path helpers ─── */

export function folderFromPath(sourcePath: string): string {
    const parts = sourcePath.split("/");
    if (parts.length <= 1) return "/";
    return parts.slice(0, -1).join("/");
}

export function filenameFromPath(sourcePath: string): string {
    return sourcePath.split("/").pop() ?? sourcePath;
}

/* ─── Folder tree ─── */

export interface FolderNode {
    name: string;
    fullPath: string;
    docs: DocumentListItem[];
    children: FolderNode[];
}

export function buildFolderTree(documents: DocumentListItem[]): FolderNode {
    const root: FolderNode = { name: "/", fullPath: "", docs: [], children: [] };

    for (const doc of documents) {
        const folder = folderFromPath(doc.sourcePath);
        const parts = folder === "/" ? [] : folder.split("/").filter(Boolean);

        let node = root;
        let path = "";
        for (const part of parts) {
            path = path ? `${path}/${part}` : part;
            let child = node.children.find(c => c.name === part);
            if (!child) {
                child = { name: part, fullPath: path, docs: [], children: [] };
                node.children.push(child);
            }
            node = child;
        }
        node.docs.push(doc);
    }

    function sortNode(node: FolderNode) {
        node.children.sort((a, b) => a.name.localeCompare(b.name));
        node.docs.sort((a, b) => a.title.localeCompare(b.title));
        for (const child of node.children) sortNode(child);
    }
    sortNode(root);

    return root;
}

/* ─── Search helpers ─── */

export function extractSnippet(content: string, query: string, contextChars = 120): string {
    const lower = content.toLowerCase();
    const idx = lower.indexOf(query.toLowerCase());
    if (idx === -1) return content.slice(0, contextChars * 2) + (content.length > contextChars * 2 ? "..." : "");

    const start = Math.max(0, idx - contextChars);
    const end = Math.min(content.length, idx + query.length + contextChars);
    let snippet = content.slice(start, end);
    if (start > 0) snippet = "..." + snippet;
    if (end < content.length) snippet = snippet + "...";
    return snippet;
}

/* ─── Staleness ─── */

export function getStaleness(doc: DocumentListItem): { label: string; color: string; tooltip: string } {
    const contentDate = doc.oldestChunkUpdatedAt ?? doc.lastChunkUpdatedAt ?? doc.updatedAt;
    const days = Math.floor((Date.now() - new Date(contentDate).getTime()) / (1000 * 60 * 60 * 24));

    const newestDate = doc.lastChunkUpdatedAt ?? doc.updatedAt;
    const oldestDate = doc.oldestChunkUpdatedAt ?? doc.updatedAt;
    const spread = Math.floor(
        (new Date(newestDate).getTime() - new Date(oldestDate).getTime()) / (1000 * 60 * 60 * 24)
    );

    if (days <= 7) {
        return { label: "Fresh", color: "text-green-600 dark:text-green-400", tooltip: "All sections updated within the last week" };
    }
    if (days <= 30) {
        const tip = spread > 14
            ? `Some sections updated recently, oldest section is ${days} days old`
            : `Last content update ${days} days ago`;
        return { label: "Recent", color: "text-yellow-600 dark:text-yellow-400", tooltip: tip };
    }
    const tip = spread > 30
        ? `Oldest section is ${days} days old, newest is ${Math.floor((Date.now() - new Date(newestDate).getTime()) / (1000 * 60 * 60 * 24))} days old`
        : `Content last updated ${days} days ago`;
    return { label: "May be outdated", color: "text-orange-600 dark:text-orange-400", tooltip: tip };
}

/* ─── Reading time ─── */

export function estimateReadingTime(chunks: DocumentChunk[]): number {
    const words = chunks.reduce((sum, c) => sum + c.content.split(/\s+/).length, 0);
    return Math.max(1, Math.ceil(words / 200));
}

/* ─── Print helper ─── */

export function mdToHtml(md: string): string {
    return md
        .replace(/^```(\w*)\n([\s\S]*?)```$/gm, "<pre><code>$2</code></pre>")
        .replace(/^### (.+)$/gm, "<h3>$1</h3>")
        .replace(/^## (.+)$/gm, "<h2>$1</h2>")
        .replace(/\*\*(.+?)\*\*/g, "<strong>$1</strong>")
        .replace(/`([^`]+)`/g, "<code>$1</code>")
        .replace(/^\| (.+) \|$/gm, row => {
            const cells = row.split("|").filter(c => c.trim()).map(c => c.trim());
            if (cells.every(c => /^-+$/.test(c))) return "";
            return `<tr>${cells.map(c => `<td>${c}</td>`).join("")}</tr>`;
        })
        .replace(/^- (.+)$/gm, "<li>$1</li>")
        .replace(/^\d+\. (.+)$/gm, "<li>$1</li>")
        .replace(/\n{2,}/g, "<br><br>");
}
