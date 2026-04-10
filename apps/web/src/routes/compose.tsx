import { useMutation } from "@tanstack/react-query";
import { createFileRoute, Link, useNavigate } from "@tanstack/react-router";
import { ArrowLeft, Check, Copy, FileText, Printer } from "lucide-react";
import { useEffect, useState } from "react";

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
        sort: (search.sort as string) || undefined,
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
    const { q, sort } = Route.useSearch();
    const navigate = useNavigate();
    const { codebaseId } = useActiveCodebase();
    const [chunks, setChunks] = useState<ComposedChunk[]>([]);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState<string | null>(null);
    const [copied, setCopied] = useState(false);

    const searchMutation = useMutation({
        mutationFn: async (clauses: any[]) =>
            unwrapEden(
                await api.api.search.query.post({
                    clauses,
                    join: "and",
                    sort: (sort as any) ?? "updated",
                    limit: 500,
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
                const result = await searchMutation.mutateAsync(clauses);
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
    }, [q, codebaseId]);

    function handleCopy() {
        const md = chunks
            .map(c => {
                const parts = [`## ${c.title}`];
                if (c.summary) parts.push(`*${c.summary}*`);
                if (c.content) parts.push(c.content);
                if (c.rationale) parts.push(`**Rationale:** ${c.rationale}`);
                return parts.join("\n\n");
            })
            .join("\n\n---\n\n");
        void navigator.clipboard.writeText(md);
        setCopied(true);
        setTimeout(() => setCopied(false), 2000);
    }

    function handlePrint() {
        window.print();
    }

    return (
        <div className="container mx-auto max-w-4xl px-4 py-8 print:py-4">
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
                    <Button variant="outline" size="sm" onClick={handleCopy} className="gap-1.5">
                        {copied ? <Check className="size-3.5" /> : <Copy className="size-3.5" />}
                        {copied ? "Copied" : "Copy all"}
                    </Button>
                    <Button variant="outline" size="sm" onClick={handlePrint} className="gap-1.5">
                        <Printer className="size-3.5" />
                        Print
                    </Button>
                </div>
            </div>

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
                    <div className="flex items-center gap-2">
                        <span className="text-xs text-muted-foreground">Filters:</span>
                        <code className="bg-muted/60 rounded px-2 py-0.5 font-mono text-xs">{q}</code>
                    </div>
                )}
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

            {!loading && chunks.length > 0 && (
                <div className="space-y-12">
                    {chunks.map((chunk, i) => (
                        <article key={chunk.id} className={i > 0 ? "border-t pt-12" : ""}>
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
                                <div className="prose prose-sm dark:prose-invert max-w-none whitespace-pre-wrap text-sm leading-relaxed">
                                    {chunk.content}
                                </div>
                            )}

                            {chunk.rationale && (
                                <div className="mt-4 rounded-md border-l-2 border-amber-500/40 bg-amber-500/5 px-4 py-3">
                                    <div className="text-xs font-semibold uppercase tracking-wider text-amber-600 dark:text-amber-400 mb-1">
                                        Rationale
                                    </div>
                                    <div className="text-sm text-muted-foreground whitespace-pre-wrap">
                                        {chunk.rationale}
                                    </div>
                                </div>
                            )}
                        </article>
                    ))}
                </div>
            )}
        </div>
    );
}
