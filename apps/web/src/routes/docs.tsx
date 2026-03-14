import { useQuery } from "@tanstack/react-query";
import { createFileRoute, Link, useSearch } from "@tanstack/react-router";
import { Book, ChevronRight, Code, Layers, Network, Terminal, Zap } from "lucide-react";
import { useEffect, useState } from "react";

import { Badge } from "@/components/ui/badge";
import { useActiveCodebase } from "@/features/codebases/use-active-codebase";
import { getUser } from "@/functions/get-user";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export const Route = createFileRoute("/docs")({
    component: DocsPage,
    validateSearch: (search: Record<string, unknown>): { section?: string; tab?: string } => ({
        section: (search.section as string) ?? undefined,
        tab: (search.tab as string) ?? undefined
    }),
    beforeLoad: async () => {
        let session = null;
        try {
            session = await getUser();
        } catch {}
        return { session };
    }
});

/* ─── User Guide content ─── */

const guidesSections = [
    {
        id: "getting-started",
        title: "Getting Started",
        icon: Zap,
        content: `## Getting Started

### What is Fubbik?

Fubbik is a local-first knowledge framework for storing, navigating, and evolving structured knowledge about your codebases. It's designed for both humans (web UI, graph visualization) and machines (CLI, MCP server, VS Code extension).

### Core Concepts

**Chunks** are the central unit — discrete pieces of knowledge like architecture decisions, conventions, runbooks, or API documentation. Each chunk has a title, content (markdown), type, tags, and optional metadata like file references and decision context.

**Connections** link chunks together as directed edges (e.g., "Auth Flow" → depends_on → "Session Management"). These form a knowledge graph you can visualize and navigate.

**Codebases** organize chunks per-project. The CLI auto-detects your codebase from the git remote. Each codebase has its own vocabulary and requirements.

### Quick Start

1. Open the dashboard and create your first chunk
2. Tag it with relevant categories
3. Create more chunks and connect them
4. View the graph to see your knowledge map
5. Use the CLI for automation: \`fubbik context --codebase myproject\`

### Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| \`Cmd+K\` | Open command palette |
| \`?\` | Show all shortcuts |
| \`n\` | Create new (context-aware) |
| \`e\` | Edit current item |
| \`Esc\` | Go back |
| \`j/k\` | Navigate lists |`
    },
    {
        id: "chunks",
        title: "Working with Chunks",
        icon: Layers,
        content: `## Working with Chunks

### Creating Chunks

Navigate to **Chunks → New** or press \`n\` on any page. Fill in:
- **Title** — a clear, descriptive name
- **Content** — markdown-formatted knowledge
- **Type** — note, document, reference, schema, or checklist
- **Tags** — categorize with tags (e.g., "backend", "auth")

**Templates** pre-fill content structure. Choose from: Convention, Architecture Decision, Runbook, or API Endpoint — or create your own.

### Decision Context

Any chunk can optionally include:
- **Rationale** — why this decision was made
- **Alternatives** — other options considered
- **Consequences** — trade-offs and impacts

This turns chunks into living Architecture Decision Records (ADRs).

### File References

Link chunks to specific files in your codebase:
- **Applies To** — glob patterns like \`src/auth/**\`
- **File References** — specific files with optional symbol anchors like \`src/auth/session.ts#SessionManager\`

These enable AI tools to know which conventions apply to which code.

### Chunk Health

The **Health** page detects:
- **Orphans** — chunks with no connections (may need linking)
- **Stale** — not updated in 30+ days but connected to recently-changed chunks
- **Thin** — very short content that may need expanding`
    },
    {
        id: "graph",
        title: "Knowledge Graph",
        icon: Network,
        content: `## Knowledge Graph

### Visualization

The graph view shows your chunks as nodes and connections as edges. Three layout algorithms:
- **Force-directed** (default) — nodes repel, connections attract. Tag grouping clusters related chunks.
- **Hierarchical** — top-down layered layout
- **Radial** — spoke pattern from the most-connected node

### Interactions

- **Click** a node to select it and see details
- **Alt+Click** two nodes to find the shortest path between them
- **Drag** group backgrounds to move all contained nodes
- **Scroll** to zoom, **drag** background to pan

### Filtering

- Filter by chunk **type** and connection **relation**
- **Search** to highlight matching nodes
- **Tag grouping** — enable tag types to cluster chunks visually
- **Show/hide ungrouped** toggle

### Path Finding

Use the **Find Path** panel (route icon, top-right) to find connections between any two chunks. The relation chain shows the path with edge directions.`
    },
    {
        id: "requirements",
        title: "Requirements",
        icon: Code,
        content: `## Requirements

### Given/When/Then

Write structured requirements in BDD format:

\`\`\`
Given a user is logged in
And they have chunks in their knowledge base
When they visit the dashboard
Then they see their chunk count
And they see recent activity
\`\`\`

### Controlled Vocabulary

Each codebase has its own vocabulary — a dictionary of valid words organized by category:
- **Actors** — user, admin, system
- **Actions** — clicks, creates, deletes
- **Targets** — chunk, codebase, tag
- **Outcomes** — sees, receives, is redirected
- **States** — logged in, on the dashboard

The step builder validates your text in real-time and highlights unknown words.

### Export Formats

Requirements export to three formats:
- **Gherkin** (\`.feature\`) — for Cucumber-style test runners
- **Vitest** (\`.test.ts\`) — TypeScript test scaffolds
- **Markdown** — checklist format

### Status Tracking

Track requirement status: **passing**, **failing**, or **untested**. Update manually or via CLI after running tests.`
    },
    {
        id: "cli",
        title: "CLI & Automation",
        icon: Terminal,
        content: `## CLI & Automation

### Installation

The CLI is at \`apps/cli/\`. It auto-detects your codebase via git remote.

### Key Commands

\`\`\`bash
# Knowledge management
fubbik add "My Chunk" --content "..." --type note
fubbik list --codebase myproject
fubbik search "auth"

# Codebase management
fubbik codebase add myproject
fubbik codebase current

# Requirements
fubbik requirements add "User login" --step "given: a user exists" --step "when: they enter credentials" --step "then: they are logged in"
fubbik requirements export --format gherkin

# AI-powered features
fubbik context --max-tokens 4000    # token-aware export
fubbik generate claude.md           # auto-generate CLAUDE.md
fubbik generate agents.md           # auto-generate AGENTS.md
\`\`\`

### MCP Server

Fubbik exposes an MCP server for AI agents. Configure in your AI tool:

\`\`\`json
{
  "mcpServers": {
    "fubbik": {
      "command": "npx",
      "args": ["tsx", "packages/mcp/src/index.ts"],
      "env": { "FUBBIK_SERVER_URL": "http://localhost:3000" }
    }
  }
}
\`\`\`

Available tools: \`search_chunks\`, \`get_chunk\`, \`create_chunk\`, \`get_conventions\`, \`get_requirements\`, \`search_vocabulary\`.

### VS Code Extension

Open the fubbik sidebar in VS Code/Cursor to browse chunks, create new ones, and view chunk details — all without leaving the editor.`
    }
];

/* ─── Developer Docs (from chunks) ─── */

function DeveloperDocs() {
    const { codebaseId } = useActiveCodebase();

    const { data, isLoading } = useQuery({
        queryKey: ["docs-chunks", codebaseId],
        queryFn: async () => {
            try {
                const result = unwrapEden(
                    await api.api.chunks.get({
                        query: { limit: "50", sort: "alpha", ...(codebaseId ? { codebaseId } : {}) } as any
                    })
                );
                return result?.chunks ?? [];
            } catch {
                return [];
            }
        }
    });

    if (isLoading) return <p className="text-muted-foreground py-8 text-center text-sm">Loading architecture docs...</p>;

    const chunks = data ?? [];
    const architecture = chunks.filter(c => c.type === "document");
    const conventions = chunks.filter(c => c.type === "note");
    const references = chunks.filter(c => c.type === "reference");
    const other = chunks.filter(c => !["document", "note", "reference"].includes(c.type));

    const sections = [
        { title: "Architecture", chunks: architecture, desc: "System design and structure" },
        { title: "Conventions", chunks: conventions, desc: "Coding standards and patterns" },
        { title: "References", chunks: references, desc: "API docs and specifications" },
        { title: "Other", chunks: other, desc: "Additional documentation" }
    ].filter(s => s.chunks.length > 0);

    if (sections.length === 0) {
        return (
            <div className="flex flex-col items-center gap-3 py-12">
                <Book className="text-muted-foreground/30 size-10" />
                <p className="text-muted-foreground text-sm">No documentation chunks found for this codebase.</p>
                <p className="text-muted-foreground text-xs">Create chunks with type "document" or "note" to populate this section.</p>
            </div>
        );
    }

    return (
        <div className="space-y-8">
            {sections.map(section => (
                <div key={section.title}>
                    <div className="mb-3">
                        <h3 className="text-lg font-semibold">{section.title}</h3>
                        <p className="text-muted-foreground text-xs">{section.desc}</p>
                    </div>
                    <div className="divide-border divide-y rounded-lg border">
                        {section.chunks.map(chunk => (
                            <Link
                                key={chunk.id}
                                to="/chunks/$chunkId"
                                params={{ chunkId: chunk.id }}
                                className="hover:bg-muted/30 group flex items-center gap-3 px-4 py-3 transition-colors"
                            >
                                <div className="min-w-0 flex-1">
                                    <p className="truncate text-sm font-medium">{chunk.title}</p>
                                    {chunk.summary && (
                                        <p className="text-muted-foreground mt-0.5 truncate text-xs">{chunk.summary}</p>
                                    )}
                                </div>
                                <Badge variant="secondary" size="sm" className="shrink-0 font-mono text-[9px]">
                                    {chunk.type}
                                </Badge>
                                <ChevronRight className="text-muted-foreground/0 group-hover:text-muted-foreground size-4 shrink-0 transition-colors" />
                            </Link>
                        ))}
                    </div>
                </div>
            ))}
        </div>
    );
}

/* ─── Simple markdown renderer ─── */

function MarkdownBlock({ content }: { content: string }) {
    const html = content
        .replace(/^### (.+)$/gm, '<h3 class="text-base font-semibold mt-6 mb-2">$1</h3>')
        .replace(/^## (.+)$/gm, '<h2 class="text-xl font-bold mt-8 mb-3">$1</h2>')
        .replace(/\*\*(.+?)\*\*/g, "<strong>$1</strong>")
        .replace(/`([^`]+)`/g, '<code class="bg-muted px-1.5 py-0.5 rounded text-[13px] font-mono">$1</code>')
        .replace(/^```(\w*)\n([\s\S]*?)```$/gm, (_m, _lang, code) =>
            `<pre class="bg-muted/50 border rounded-lg p-4 text-[13px] font-mono overflow-x-auto my-3 leading-relaxed"><code>${code.trim()}</code></pre>`
        )
        .replace(/^\| (.+) \|$/gm, (row) => {
            const cells = row.split("|").filter(c => c.trim()).map(c => c.trim());
            const isHeader = cells.every(c => /^-+$/.test(c));
            if (isHeader) return "";
            const tag = row.includes("---") ? "th" : "td";
            return `<tr>${cells.map(c => `<${tag} class="border px-3 py-1.5 text-sm text-left">${c}</${tag}>`).join("")}</tr>`;
        })
        .replace(/(<tr>[\s\S]*?<\/tr>)/g, (match, _p1, offset, str) => {
            const before = str.substring(0, offset);
            if (!before.includes("<table>") || before.lastIndexOf("</table>") > before.lastIndexOf("<table>")) {
                return `<table class="border-collapse border rounded my-3 w-full">${match}`;
            }
            const after = str.substring(offset + match.length);
            if (!after.match(/^\s*<tr>/)) {
                return `${match}</table>`;
            }
            return match;
        })
        .replace(/^- (.+)$/gm, '<li class="text-sm ml-4 list-disc mb-1">$1</li>')
        .replace(/^\d+\. (.+)$/gm, '<li class="text-sm ml-4 list-decimal mb-1">$1</li>')
        .replace(/\n{2,}/g, '<div class="h-3"></div>');

    return (
        <div
            className="text-foreground/90 text-sm leading-relaxed"
            dangerouslySetInnerHTML={{ __html: html }}
        />
    );
}

/* ─── Main page ─── */

function DocsPage() {
    const search = useSearch({ from: "/docs" });
    const [tab, setTab] = useState<"guide" | "dev">(search.tab === "dev" ? "dev" : "guide");
    const [activeSection, setActiveSection] = useState(
        search.section && guidesSections.some(s => s.id === search.section)
            ? search.section
            : guidesSections[0]!.id
    );

    // Respond to search param changes (e.g., navigating from landing page)
    useEffect(() => {
        if (search.section && guidesSections.some(s => s.id === search.section)) {
            setActiveSection(search.section);
            setTab("guide");
        }
        if (search.tab === "dev") {
            setTab("dev");
        }
    }, [search.section, search.tab]);

    const currentGuide = guidesSections.find(s => s.id === activeSection) ?? guidesSections[0]!;

    return (
        <div className="container mx-auto max-w-6xl px-4 py-8">
            {/* Header */}
            <div className="mb-6">
                <h1 className="text-2xl font-bold tracking-tight">Documentation</h1>
                <p className="text-muted-foreground mt-0.5 text-sm">Learn how to use fubbik and explore your codebase knowledge.</p>
            </div>

            {/* Tabs */}
            <div className="border-border mb-6 flex gap-1 border-b">
                <button
                    onClick={() => setTab("guide")}
                    className={`-mb-px border-b-2 px-4 py-2 text-sm font-medium transition-colors ${
                        tab === "guide"
                            ? "border-foreground text-foreground"
                            : "text-muted-foreground hover:text-foreground border-transparent"
                    }`}
                >
                    <span className="flex items-center gap-2">
                        <Book className="size-4" />
                        User Guide
                    </span>
                </button>
                <button
                    onClick={() => setTab("dev")}
                    className={`-mb-px border-b-2 px-4 py-2 text-sm font-medium transition-colors ${
                        tab === "dev"
                            ? "border-foreground text-foreground"
                            : "text-muted-foreground hover:text-foreground border-transparent"
                    }`}
                >
                    <span className="flex items-center gap-2">
                        <Code className="size-4" />
                        Developer Docs
                    </span>
                </button>
            </div>

            {/* Content */}
            {tab === "guide" ? (
                <div className="grid gap-8 lg:grid-cols-[220px_1fr]">
                    {/* Sidebar nav */}
                    <nav className="hidden lg:block">
                        <div className="sticky top-24 space-y-1">
                            {guidesSections.map(section => (
                                <button
                                    key={section.id}
                                    onClick={() => setActiveSection(section.id)}
                                    className={`flex w-full items-center gap-2 rounded-md px-3 py-2 text-left text-sm transition-colors ${
                                        activeSection === section.id
                                            ? "bg-muted text-foreground font-medium"
                                            : "text-muted-foreground hover:text-foreground hover:bg-muted/50"
                                    }`}
                                >
                                    <section.icon className="size-4 shrink-0" />
                                    {section.title}
                                </button>
                            ))}
                        </div>
                    </nav>

                    {/* Mobile section selector */}
                    <div className="mb-4 flex gap-2 overflow-x-auto lg:hidden">
                        {guidesSections.map(section => (
                            <button
                                key={section.id}
                                onClick={() => setActiveSection(section.id)}
                                className={`shrink-0 rounded-full px-3 py-1 text-xs font-medium transition-colors ${
                                    activeSection === section.id
                                        ? "bg-foreground text-background"
                                        : "bg-muted text-muted-foreground"
                                }`}
                            >
                                {section.title}
                            </button>
                        ))}
                    </div>

                    {/* Guide content */}
                    <div className="min-w-0">
                        <MarkdownBlock content={currentGuide!.content} />
                    </div>
                </div>
            ) : (
                <DeveloperDocs />
            )}
        </div>
    );
}
