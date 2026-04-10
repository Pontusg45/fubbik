import { useMutation } from "@tanstack/react-query";
import { createFileRoute, Link, useNavigate } from "@tanstack/react-router";
import { ArrowLeft, Check, Copy, Download, FileText, Printer } from "lucide-react";
import { useEffect, useMemo, useState } from "react";

import { MarkdownRenderer } from "@/components/markdown-renderer";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { useActiveCodebase } from "@/features/codebases/use-active-codebase";
import { getUser } from "@/functions/get-user";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export const Route = createFileRoute("/compose")({
    component: ComposePage,
    validateSearch: (search: Record<string, unknown>) => ({
        q: (search.q as string) || undefined,
        sort: (search.sort as string) || "updated",
        group: (search.group as string) || "none",
        limit: (search.limit as string) || "50",
    }),
    beforeLoad: async () => {
        let session = null;
        try {
            session = await getUser();
        } catch {
            // allow guest
        }
        return { session };
    },
});

interface ComposedChunk {
    id: string;
    title: string;
    type: string;
    summary: string | null;
    content?: string;
    tags?: string[];
    connectionCount?: number;
    rationale?: string | null;
}

/** Parse a simple query string like `type:reference tag:api` into clauses. */
function parseSimpleQuery(q: string): Array<{ field: string; operator: string; value: string; negate?: boolean }> {
    const clauses: Array<{ field: string; operator: string; value: string; negate?: boolean }> = [];
    const tokens = q.trim().split(/\s+/).filter(Boolean);
    let negate = false;
    for (const token of tokens) {
        if (token.toUpperCase() === "NOT") {
            negate = true;
            continue;
        }
        const colonIdx = token.indexOf(":");
        if (colonIdx > 0) {
            const field = token.slice(0, colonIdx);
            const value = token.slice(colonIdx + 1);
            clauses.push({ field, operator: "is", value, negate });
        } else {
            clauses.push({ field: "text", operator: "contains", value: token, negate });
        }
        negate = false;
    }
    return clauses;
}

function ComposePage() {
    const { q, sort, group, limit } = Route.useSearch();
    const navigate = useNavigate();
    const { codebaseId } = useActiveCodebase();
    const [chunks, setChunks] = useState<ComposedChunk[]>([]);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState<string | null>(null);
    const [copied, setCopied] = useState(false);
    const [scrollProgress, setScrollProgress] = useState(0);

    function updateParam(key: string, value: string) {
        void navigate({
            to: "/compose",
            search: { q, sort, group, limit, [key]: value } as any,
            replace: true,
        });
    }

    useEffect(() => {
        document.documentElement.classList.add("scroll-smooth");
        return () => document.documentElement.classList.remove("scroll-smooth");
    }, []);

    const searchMutation = useMutation({
        mutationFn: async ({ clauses, lim }: { clauses: any[]; lim: number }) =>
            unwrapEden(
                await api.api.search.query.post({
                    clauses,
                    join: "and",
                    sort: (sort as any) ?? "updated",
                    limit: lim,
                    offset: 0,
                    ...(codebaseId ? { codebaseId } : {}),
                } as any),
            ),
    });

    useEffect(() => {
        async function load() {
            if (!q) {
                setLoading(false);
                setChunks([]);
                return;
            }
            try {
                setLoading(true);
                setError(null);
                const clauses = parseSimpleQuery(q);
                const lim = limit === "all" ? 500 : Number(limit);
                const result = await searchMutation.mutateAsync({ clauses, lim });
                const searchChunks = (result as any)?.chunks ?? [];

                // Fetch full content for each chunk (search only returns summary)
                const full = await Promise.all(
                    searchChunks.map(async (c: any) => {
                        try {
                            const detail = unwrapEden(await api.api.chunks({ id: c.id }).get());
                            return {
                                id: c.id,
                                title: c.title,
                                type: c.type,
                                summary: c.summary,
                                content: (detail as any)?.content ?? "",
                                tags: ((detail as any)?.tags ?? []).map((t: any) => t.name ?? t),
                                connectionCount: c.connectionCount ?? 0,
                                rationale: (detail as any)?.rationale ?? null,
                            } as ComposedChunk;
                        } catch {
                            return {
                                id: c.id,
                                title: c.title,
                                type: c.type,
                                summary: c.summary,
                                content: "",
                                tags: [],
                                connectionCount: c.connectionCount ?? 0,
                            } as ComposedChunk;
                        }
                    }),
                );
                setChunks(full);
            } catch (e: any) {
                setError(e?.message ?? "Failed to load chunks");
                setChunks([]);
            } finally {
                setLoading(false);
            }
        }
        void load();
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [q, codebaseId, limit]);

    const sortedChunks = useMemo(() => {
        const copy = [...chunks];
        switch (sort) {
            case "title":
                return copy.sort((a, b) => a.title.localeCompare(b.title));
            case "type":
                return copy.sort((a, b) => a.type.localeCompare(b.type));
            case "connections":
                return copy.sort((a, b) => (b.connectionCount ?? 0) - (a.connectionCount ?? 0));
            default:
                return copy;
        }
    }, [chunks, sort]);

    const groupedChunks = useMemo(() => {
        if (group === "none") return null;
        const groups = new Map<string, ComposedChunk[]>();
        for (const chunk of sortedChunks) {
            let key: string;
            if (group === "type") {
                key = chunk.type;
            } else if (group === "tag") {
                key = chunk.tags && chunk.tags.length > 0 ? (chunk.tags[0] ?? "untagged") : "untagged";
            } else {
                key = "all";
            }
            const existing = groups.get(key) ?? [];
            existing.push(chunk);
            groups.set(key, existing);
        }
        return Array.from(groups.entries()).sort(([a], [b]) => a.localeCompare(b));
    }, [sortedChunks, group]);

    function buildMarkdown(): string {
        const filterLine = q ? `<!-- Filters: ${q} -->\n\n` : "";
        const body = sortedChunks
            .map(c => {
                const parts = [`## ${c.title}`];
                if (c.summary) parts.push(`*${c.summary}*`);
                if (c.content) parts.push(c.content);
                if (c.rationale) parts.push(`**Rationale:** ${c.rationale}`);
                return parts.join("\n\n");
            })
            .join("\n\n---\n\n");
        return `# Composite View\n\n${filterLine}${body}`;
    }

    function handleCopy() {
        void navigator.clipboard.writeText(buildMarkdown());
        setCopied(true);
        setTimeout(() => setCopied(false), 2000);
    }

    function handleDownload() {
        const md = buildMarkdown();
        const blob = new Blob([md], { type: "text/markdown" });
        const url = URL.createObjectURL(blob);
        const a = document.createElement("a");
        a.href = url;
        a.download = `composite-${new Date().toISOString().slice(0, 10)}.md`;
        a.click();
        URL.revokeObjectURL(url);
    }

    function removeFilter(index: number) {
        const clauses = parseSimpleQuery(q ?? "");
        const remaining = clauses.filter((_, i) => i !== index);
        const newQ = remaining
            .map(c => `${c.negate ? "NOT " : ""}${c.field}:${c.value}`)
            .join(" ");
        void navigate({
            to: "/compose",
            search: { q: newQ || undefined, sort, group, limit } as any,
            replace: true,
        });
    }

    function handlePrint() {
        window.print();
    }

    useEffect(() => {
        function handleKey(e: KeyboardEvent) {
            // Skip if user is typing in an input
            const tag = document.activeElement?.tagName;
            if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT" || (document.activeElement as HTMLElement)?.isContentEditable) {
                return;
            }

            if (e.key !== "j" && e.key !== "k") return;
            if (sortedChunks.length === 0) return;

            const articles = sortedChunks
                .map(c => document.getElementById(`chunk-${c.id}`))
                .filter((el): el is HTMLElement => el !== null);
            if (articles.length === 0) return;

            const viewportTop = window.scrollY + 100;
            let currentIdx = 0;
            for (let i = 0; i < articles.length; i++) {
                if (articles[i]!.offsetTop <= viewportTop) {
                    currentIdx = i;
                }
            }

            let targetIdx = currentIdx;
            if (e.key === "j") targetIdx = Math.min(sortedChunks.length - 1, currentIdx + 1);
            if (e.key === "k") targetIdx = Math.max(0, currentIdx - 1);

            const target = articles[targetIdx];
            if (target) {
                target.scrollIntoView({ behavior: "smooth", block: "start" });
            }
        }

        window.addEventListener("keydown", handleKey);
        return () => window.removeEventListener("keydown", handleKey);
    }, [sortedChunks]);

    useEffect(() => {
        function handleScroll() {
            const scrollTop = window.scrollY;
            const docHeight = document.documentElement.scrollHeight - window.innerHeight;
            const progress = docHeight > 0 ? (scrollTop / docHeight) * 100 : 0;
            setScrollProgress(Math.min(100, Math.max(0, progress)));
        }
        window.addEventListener("scroll", handleScroll, { passive: true });
        handleScroll();
        return () => window.removeEventListener("scroll", handleScroll);
    }, [chunks]);

    function renderChunk(chunk: ComposedChunk, i: number, withBorder: boolean) {
        return (
            <article
                key={chunk.id}
                id={`chunk-${chunk.id}`}
                className={`${withBorder && i > 0 ? "border-t pt-12" : ""} ${i > 0 ? "print:break-before-page" : ""} print:pt-0 print:border-0`.trim()}
            >
                <div className="mb-4">
                    <Link
                        to="/chunks/$chunkId"
                        params={{ chunkId: chunk.id }}
                        className="group inline-flex items-baseline gap-3 hover:underline"
                    >
                        <h2 className="text-2xl font-bold tracking-tight group-hover:text-primary transition-colors">
                            {chunk.title}
                        </h2>
                    </Link>
                    <div className="mt-2 flex flex-wrap items-center gap-2">
                        <Badge variant="secondary" size="sm" className="font-mono text-[9px]">
                            {chunk.type}
                        </Badge>
                        {chunk.tags && chunk.tags.length > 0 && (
                            <span className="text-xs text-muted-foreground">
                                {chunk.tags.join(" · ")}
                            </span>
                        )}
                        {chunk.connectionCount ? (
                            <span className="text-xs text-muted-foreground">
                                · {chunk.connectionCount} connection{chunk.connectionCount === 1 ? "" : "s"}
                            </span>
                        ) : null}
                    </div>
                </div>

                {chunk.summary && (
                    <p className="mb-4 text-base italic text-muted-foreground">{chunk.summary}</p>
                )}

                {chunk.content && (
                    <div className="prose prose-sm dark:prose-invert max-w-none">
                        <MarkdownRenderer>{chunk.content}</MarkdownRenderer>
                    </div>
                )}

                {chunk.rationale && (
                    <div className="mt-4 rounded-md border-l-2 border-amber-500/40 bg-amber-500/5 px-4 py-3">
                        <div className="text-xs font-semibold uppercase tracking-wider text-amber-600 dark:text-amber-400 mb-1">
                            Rationale
                        </div>
                        <div className="prose prose-sm dark:prose-invert max-w-none text-muted-foreground">
                            <MarkdownRenderer>{chunk.rationale}</MarkdownRenderer>
                        </div>
                    </div>
                )}
            </article>
        );
    }

    return (
        <>
        <div className="fixed top-0 left-0 right-0 z-50 h-0.5 bg-transparent print:hidden">
            <div
                className="h-full bg-primary transition-[width] duration-100 ease-out"
                style={{ width: `${scrollProgress}%` }}
            />
        </div>
        <div className="container mx-auto max-w-6xl px-4 py-8 print:py-4">
            <style>{`
                @media print {
                    body { font-family: Georgia, 'Times New Roman', serif; }
                    .prose { max-width: none !important; }
                    a { color: inherit; text-decoration: none; }
                    h1, h2, h3 { page-break-after: avoid; }
                    article { page-break-inside: avoid; }
                }
            `}</style>
            {/* Header */}
            <div className="mb-6 flex items-center justify-between print:hidden">
                <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => void navigate({ to: "/search", search: { q } as any })}
                    className="gap-1.5"
                >
                    <ArrowLeft className="size-3.5" />
                    Back to search
                </Button>
                <div className="flex items-center gap-2">
                    <span className="hidden lg:inline text-[10px] text-muted-foreground/60 font-mono mr-2">
                        press <kbd className="rounded border bg-muted px-1 py-0.5">j</kbd> / <kbd className="rounded border bg-muted px-1 py-0.5">k</kbd> to navigate
                    </span>
                    <Button variant="outline" size="sm" onClick={handleCopy} className="gap-1.5">
                        {copied ? <Check className="size-3.5" /> : <Copy className="size-3.5" />}
                        {copied ? "Copied" : "Copy all"}
                    </Button>
                    <Button variant="outline" size="sm" onClick={handleDownload} className="gap-1.5">
                        <Download className="size-3.5" />
                        Download
                    </Button>
                    <Button variant="outline" size="sm" onClick={handlePrint} className="gap-1.5">
                        <Printer className="size-3.5" />
                        Print
                    </Button>
                </div>
            </div>

            {/* Two-column layout */}
            <div className="flex gap-8">
                {/* ToC sidebar */}
                <aside className="hidden lg:block w-56 shrink-0 print:hidden">
                    <div className="sticky top-8">
                        <div className="text-xs font-semibold uppercase tracking-wider text-muted-foreground mb-3">
                            Contents
                        </div>
                        {sortedChunks.length > 0 ? (
                            <nav className="space-y-1 max-h-[calc(100vh-8rem)] overflow-y-auto">
                                {sortedChunks.map((chunk) => (
                                    <a
                                        key={chunk.id}
                                        href={`#chunk-${chunk.id}`}
                                        className="block truncate rounded px-2 py-1 text-xs text-muted-foreground hover:bg-muted hover:text-foreground transition-colors"
                                        title={chunk.title}
                                    >
                                        {chunk.title}
                                    </a>
                                ))}
                            </nav>
                        ) : (
                            <p className="text-xs text-muted-foreground">No chunks</p>
                        )}
                    </div>
                </aside>

                {/* Main content column */}
                <div className="flex-1 min-w-0">
                    {/* Title and filter summary */}
                    <div className="mb-8 border-b pb-6">
                        <div className="flex items-center gap-2 text-xs text-muted-foreground uppercase tracking-wider mb-2">
                            <FileText className="size-3.5" />
                            Composite View
                        </div>
                        <h1 className="text-3xl font-bold tracking-tight mb-3">
                            {loading ? "Loading…" : `${chunks.length} chunk${chunks.length === 1 ? "" : "s"}`}
                        </h1>
                        {q && (
                            <div className="flex flex-wrap items-center gap-2">
                                <span className="text-xs text-muted-foreground">Filters:</span>
                                {parseSimpleQuery(q).map((clause, idx) => (
                                    <span
                                        key={idx}
                                        className="inline-flex items-center gap-1 rounded border border-slate-500/30 bg-slate-500/15 text-slate-400 px-2 py-0.5 text-xs"
                                    >
                                        {clause.negate && <span className="font-semibold">NOT</span>}
                                        <span className="font-semibold">{clause.field}</span>
                                        <span className="text-muted-foreground">is</span>
                                        <span>{clause.value}</span>
                                        <button
                                            type="button"
                                            onClick={() => removeFilter(idx)}
                                            className="opacity-50 hover:opacity-100 transition-opacity print:hidden"
                                            aria-label={`Remove ${clause.field} filter`}
                                        >
                                            ×
                                        </button>
                                    </span>
                                ))}
                            </div>
                        )}
                        <div className="flex items-center gap-3 mt-3 text-xs print:hidden">
                            <label className="flex items-center gap-1.5 text-muted-foreground">
                                Sort:
                                <select
                                    value={sort}
                                    onChange={e => updateParam("sort", e.target.value)}
                                    className="bg-muted/50 rounded px-2 py-1 border text-xs"
                                >
                                    <option value="updated">Recently updated</option>
                                    <option value="newest">Newest</option>
                                    <option value="oldest">Oldest</option>
                                    <option value="title">Title A-Z</option>
                                    <option value="type">By type</option>
                                    <option value="connections">Most connected</option>
                                </select>
                            </label>
                            <label className="flex items-center gap-1.5 text-muted-foreground">
                                Group by:
                                <select
                                    value={group}
                                    onChange={e => updateParam("group", e.target.value)}
                                    className="bg-muted/50 rounded px-2 py-1 border text-xs"
                                >
                                    <option value="none">None</option>
                                    <option value="type">Type</option>
                                    <option value="tag">Tag</option>
                                </select>
                            </label>
                            <label className="flex items-center gap-1.5 text-muted-foreground">
                                Limit:
                                <select
                                    value={limit}
                                    onChange={e => updateParam("limit", e.target.value)}
                                    className="bg-muted/50 rounded px-2 py-1 border text-xs"
                                >
                                    <option value="10">10</option>
                                    <option value="25">25</option>
                                    <option value="50">50</option>
                                    <option value="100">100</option>
                                    <option value="all">All</option>
                                </select>
                            </label>
                        </div>
                    </div>

                    {/* Content */}
                    {loading && (
                        <div className="py-16 text-center text-muted-foreground">Loading chunks…</div>
                    )}

                    {error && (
                        <div className="rounded-lg border border-red-500/20 bg-red-500/5 p-4 text-sm text-red-500">
                            {error}
                        </div>
                    )}

                    {!loading && !error && chunks.length === 0 && (
                        <div className="py-16 text-center text-muted-foreground">
                            No chunks match the current filter.
                        </div>
                    )}

                    {!loading && chunks.length > 0 && groupedChunks && (
                        <div className="space-y-16">
                            {groupedChunks.map(([groupKey, groupChunks]) => (
                                <section key={groupKey}>
                                    <h2 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground mb-6 pb-2 border-b">
                                        {groupKey} <span className="text-muted-foreground/60">({groupChunks.length})</span>
                                    </h2>
                                    <div className="space-y-12">
                                        {groupChunks.map((chunk, i) => renderChunk(chunk, i, true))}
                                    </div>
                                </section>
                            ))}
                        </div>
                    )}

                    {!loading && chunks.length > 0 && !groupedChunks && (
                        <div className="space-y-12">
                            {sortedChunks.map((chunk, i) => renderChunk(chunk, i, true))}
                        </div>
                    )}
                </div>
            </div>
        </div>
        </>
    );
}
