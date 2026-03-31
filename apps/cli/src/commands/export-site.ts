import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { Command } from "commander";
import { formatSuccess } from "../lib/colors";
import { output, outputError } from "../lib/output";
import { getServerUrl } from "../lib/store";

function escapeHtml(str: string): string {
    return str.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
}

function markdownToHtml(md: string): string {
    // Minimal markdown-to-HTML for chunk content
    return md
        .replace(/```(\w*)\n([\s\S]*?)```/g, "<pre><code>$2</code></pre>")
        .replace(/^### (.+)$/gm, "<h3>$1</h3>")
        .replace(/^## (.+)$/gm, "<h2>$1</h2>")
        .replace(/^# (.+)$/gm, "<h1>$1</h1>")
        .replace(/\*\*(.+?)\*\*/g, "<strong>$1</strong>")
        .replace(/`([^`]+)`/g, "<code>$1</code>")
        .replace(/^- (.+)$/gm, "<li>$1</li>")
        .replace(/(<li>.*<\/li>\n?)+/g, "<ul>$&</ul>")
        .replace(/\n\n/g, "</p><p>")
        .replace(/^(?!<[huplo])/gm, "")
        .split("\n")
        .filter(Boolean)
        .join("\n");
}

const CSS = `
* { box-sizing: border-box; margin: 0; padding: 0; }
body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif; line-height: 1.6; color: #1a1a2e; background: #fafafa; max-width: 900px; margin: 0 auto; padding: 2rem 1rem; }
a { color: #3b82f6; text-decoration: none; }
a:hover { text-decoration: underline; }
h1 { font-size: 1.8rem; margin-bottom: 0.5rem; }
h2 { font-size: 1.3rem; margin: 1.5rem 0 0.5rem; border-bottom: 1px solid #e5e7eb; padding-bottom: 0.25rem; }
h3 { font-size: 1.1rem; margin: 1rem 0 0.25rem; }
p { margin: 0.5rem 0; }
code { background: #f1f5f9; padding: 0.15rem 0.35rem; border-radius: 3px; font-size: 0.9em; }
pre { background: #1e293b; color: #e2e8f0; padding: 1rem; border-radius: 6px; overflow-x: auto; margin: 1rem 0; }
pre code { background: none; color: inherit; padding: 0; }
ul { padding-left: 1.5rem; margin: 0.5rem 0; }
.badge { display: inline-block; padding: 0.15rem 0.5rem; border-radius: 9999px; font-size: 0.75rem; font-weight: 500; }
.type { background: #dbeafe; color: #1e40af; }
.tag { background: #fef3c7; color: #92400e; margin: 0 0.15rem; }
.chunk-card { border: 1px solid #e5e7eb; border-radius: 8px; padding: 1rem 1.25rem; margin-bottom: 0.75rem; background: white; transition: border-color 0.15s; }
.chunk-card:hover { border-color: #3b82f6; }
.chunk-card h3 { margin: 0 0 0.25rem; }
.chunk-card p { color: #6b7280; font-size: 0.875rem; margin: 0; }
.meta { color: #9ca3af; font-size: 0.8rem; margin-top: 0.5rem; }
.search { width: 100%; padding: 0.6rem 1rem; border: 1px solid #e5e7eb; border-radius: 6px; font-size: 1rem; margin-bottom: 1rem; }
.filters { display: flex; gap: 0.5rem; flex-wrap: wrap; margin-bottom: 1rem; }
.filter-btn { padding: 0.3rem 0.75rem; border: 1px solid #e5e7eb; border-radius: 9999px; background: white; cursor: pointer; font-size: 0.8rem; }
.filter-btn.active { background: #3b82f6; color: white; border-color: #3b82f6; }
.header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 1.5rem; border-bottom: 1px solid #e5e7eb; padding-bottom: 1rem; }
.back { font-size: 0.9rem; }
.content { margin-top: 1rem; }
.rationale { background: #f0fdf4; border-left: 3px solid #22c55e; padding: 0.75rem 1rem; margin: 1rem 0; border-radius: 0 6px 6px 0; }
.nav { display: flex; gap: 1rem; align-items: center; }
`;

function generateIndex(chunks: any[]): string {
    const types = [...new Set(chunks.map((c: any) => c.type))];
    const typeButtons = types
        .map((t) => `<button class="filter-btn" onclick="filterByType('${t}')">${t}</button>`)
        .join("");

    const cards = chunks
        .map((c: any) => {
            const tags = (c.tags ?? [])
                .map((t: any) => {
                    const name = typeof t === "string" ? t : t.name;
                    return `<span class="badge tag">${escapeHtml(name)}</span>`;
                })
                .join(" ");
            const preview = escapeHtml((c.content ?? "").slice(0, 150)).replace(/\n/g, " ");
            return `<div class="chunk-card" data-type="${escapeHtml(c.type)}" data-title="${escapeHtml(c.title.toLowerCase())}">
    <h3><a href="chunk-${c.id}.html">${escapeHtml(c.title)}</a></h3>
    <p>${preview}${(c.content?.length ?? 0) > 150 ? "..." : ""}</p>
    <div class="meta"><span class="badge type">${escapeHtml(c.type)}</span> ${tags}</div>
</div>`;
        })
        .join("\n");

    return `<!DOCTYPE html>
<html lang="en">
<head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>fubbik Knowledge Base</title><style>${CSS}</style></head>
<body>
<div class="header">
    <h1>fubbik Knowledge Base</h1>
    <div class="nav"><span class="meta">${chunks.length} chunks</span></div>
</div>
<input type="text" class="search" placeholder="Search chunks..." oninput="filterChunks(this.value)">
<div class="filters"><button class="filter-btn active" onclick="filterByType('')">All</button>${typeButtons}</div>
<div id="chunks">${cards}</div>
<script>
let activeType = '';
function filterChunks(query) {
    const q = query.toLowerCase();
    document.querySelectorAll('.chunk-card').forEach(el => {
        const matchesSearch = !q || el.dataset.title.includes(q) || el.textContent.toLowerCase().includes(q);
        const matchesType = !activeType || el.dataset.type === activeType;
        el.style.display = matchesSearch && matchesType ? '' : 'none';
    });
}
function filterByType(type) {
    activeType = type;
    document.querySelectorAll('.filter-btn').forEach(b => b.classList.remove('active'));
    event.target.classList.add('active');
    filterChunks(document.querySelector('.search').value);
}
</script>
</body></html>`;
}

function generateChunkPage(chunk: any): string {
    const tags = (chunk.tags ?? [])
        .map((t: any) => {
            const name = typeof t === "string" ? t : t.name;
            return `<span class="badge tag">${escapeHtml(name)}</span>`;
        })
        .join(" ");

    const content = markdownToHtml(chunk.content ?? "");

    let extras = "";
    if (chunk.rationale) {
        extras += `<div class="rationale"><strong>Rationale:</strong> ${escapeHtml(chunk.rationale)}</div>`;
    }
    if (chunk.alternatives?.length) {
        extras += `<h2>Alternatives Considered</h2><ul>${chunk.alternatives.map((a: string) => `<li>${escapeHtml(a)}</li>`).join("")}</ul>`;
    }
    if (chunk.consequences) {
        extras += `<h2>Consequences</h2><p>${escapeHtml(chunk.consequences)}</p>`;
    }

    return `<!DOCTYPE html>
<html lang="en">
<head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>${escapeHtml(chunk.title)} -- fubbik</title><style>${CSS}</style></head>
<body>
<div class="header">
    <a href="index.html" class="back">&larr; Back to index</a>
    <div class="nav"><span class="badge type">${escapeHtml(chunk.type)}</span> ${tags}</div>
</div>
<h1>${escapeHtml(chunk.title)}</h1>
<div class="meta">Updated: ${new Date(chunk.updatedAt).toLocaleDateString()}</div>
<div class="content">${content}</div>
${extras}
</body></html>`;
}

export const exportSiteCommand = new Command("export-site")
    .description("Generate a static HTML site from the knowledge base")
    .option("-o, --output <dir>", "output directory", "fubbik-site")
    .option("--codebase <name>", "filter by codebase")
    .action(async (opts: { output: string; codebase?: string }, cmd: Command) => {
        const serverUrl = getServerUrl();
        if (!serverUrl) {
            outputError("Server URL required. Run 'fubbik init --server <url>'.");
            process.exit(1);
        }

        // Fetch all chunks
        const params = new URLSearchParams({ limit: "500", sort: "alpha" });
        if (opts.codebase) {
            const cbRes = await fetch(`${serverUrl}/api/codebases`);
            if (cbRes.ok) {
                const codebases = (await cbRes.json()) as { id: string; name: string }[];
                const match = codebases.find((c) => c.name === opts.codebase);
                if (match) params.set("codebaseId", match.id);
            }
        }

        const res = await fetch(`${serverUrl}/api/chunks?${params}`);
        if (!res.ok) {
            outputError(`Failed to fetch chunks: ${res.status}`);
            process.exit(1);
        }

        const data = (await res.json()) as { chunks?: any[] } | any[];
        const chunks = Array.isArray(data) ? data : (data.chunks ?? []);

        if (chunks.length === 0) {
            outputError("No chunks found.");
            process.exit(1);
        }

        // Create output directory
        mkdirSync(opts.output, { recursive: true });

        // Generate index
        writeFileSync(join(opts.output, "index.html"), generateIndex(chunks));

        // Generate individual pages
        for (const chunk of chunks) {
            writeFileSync(join(opts.output, `chunk-${chunk.id}.html`), generateChunkPage(chunk));
        }

        console.log(formatSuccess(`Generated ${chunks.length + 1} pages to ${opts.output}/`));
        console.log(`  Open: file://${join(process.cwd(), opts.output, "index.html")}`);

        output(cmd, { output: opts.output, pages: chunks.length + 1 }, "");
    });
