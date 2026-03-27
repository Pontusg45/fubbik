import { useQuery } from "@tanstack/react-query";
import { createFileRoute, Link, useNavigate } from "@tanstack/react-router";
import { Blocks, BookOpen, ClipboardCheck, FileText, Search as SearchIcon } from "lucide-react";
import { useEffect, useRef } from "react";

import { Badge } from "@/components/ui/badge";
import { useActiveCodebase } from "@/features/codebases/use-active-codebase";
import { getUser } from "@/functions/get-user";
import { useDebouncedValue } from "@/hooks/use-debounced-value";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export const Route = createFileRoute("/search")({
    component: SearchPage,
    validateSearch: (search: Record<string, unknown>) => ({
        q: (search.q as string) || undefined,
    }),
    beforeLoad: async () => {
        let session = null;
        try {
            session = await getUser();
        } catch {
            // allow guest access
        }
        return { session };
    },
});

function timeAgo(dateStr: string): string {
    const diff = Date.now() - new Date(dateStr).getTime();
    const minutes = Math.floor(diff / 60000);
    if (minutes < 1) return "just now";
    if (minutes < 60) return `${minutes}m ago`;
    const hours = Math.floor(minutes / 60);
    if (hours < 24) return `${hours}h ago`;
    const days = Math.floor(hours / 24);
    if (days < 30) return `${days}d ago`;
    const months = Math.floor(days / 30);
    return `${months}mo ago`;
}

function SearchPage() {
    const { q } = Route.useSearch();
    const navigate = useNavigate();
    const inputRef = useRef<HTMLInputElement>(null);
    const { codebaseId } = useActiveCodebase();

    const localQuery = q ?? "";
    const debouncedQuery = useDebouncedValue(localQuery, 300);
    const searchActive = debouncedQuery && debouncedQuery.length >= 2;

    // Auto-focus input on mount
    useEffect(() => {
        inputRef.current?.focus();
    }, []);

    function handleInputChange(value: string) {
        void navigate({
            to: "/search",
            search: value ? { q: value } : { q: undefined },
            replace: true,
        } as any);
    }

    // Chunks search
    const chunksQuery = useQuery({
        queryKey: ["search-chunks", debouncedQuery, codebaseId],
        queryFn: async () => {
            const query: { search: string; limit: string; codebaseId?: string } = {
                search: debouncedQuery!,
                limit: "10",
            };
            if (codebaseId) query.codebaseId = codebaseId;
            return unwrapEden(await api.api.chunks.get({ query }));
        },
        enabled: !!searchActive,
    });

    // Requirements
    const requirementsQuery = useQuery({
        queryKey: ["search-requirements"],
        queryFn: async () => {
            try {
                return unwrapEden(await api.api.requirements.get({ query: { limit: "50" } }));
            } catch {
                return { requirements: [], total: 0 };
            }
        },
        enabled: !!searchActive,
    });

    // Templates
    const templatesQuery = useQuery({
        queryKey: ["search-templates"],
        queryFn: async () => {
            try {
                return unwrapEden(await api.api.templates.get()) as Array<{
                    id: string;
                    name: string;
                    description: string | null;
                    type: string;
                }>;
            } catch {
                return [];
            }
        },
        enabled: !!searchActive,
        staleTime: 60_000,
    });

    // Vocabulary
    const vocabularyQuery = useQuery({
        queryKey: ["search-vocabulary", codebaseId],
        queryFn: async () => {
            if (!codebaseId) return [];
            try {
                return unwrapEden(
                    await api.api.vocabulary.get({ query: { codebaseId } })
                ) as Array<{ id: string; word: string; category: string }>;
            } catch {
                return [];
            }
        },
        enabled: !!searchActive && !!codebaseId,
    });

    // Filter results client-side
    const lowerQ = (debouncedQuery ?? "").toLowerCase();

    const chunks = (chunksQuery.data as { chunks: Array<Record<string, unknown>> } | undefined)?.chunks ?? [];

    const allRequirements = (
        requirementsQuery.data as { requirements: Array<Record<string, unknown>> } | undefined
    )?.requirements ?? [];
    const requirements = searchActive
        ? allRequirements.filter((r) =>
              ((r.title as string) ?? "").toLowerCase().includes(lowerQ)
          )
        : [];

    const allTemplates = Array.isArray(templatesQuery.data) ? templatesQuery.data : [];
    const templates = searchActive
        ? allTemplates.filter(
              (t) =>
                  t.name.toLowerCase().includes(lowerQ) ||
                  (t.description ?? "").toLowerCase().includes(lowerQ)
          )
        : [];

    const allVocabulary = Array.isArray(vocabularyQuery.data) ? vocabularyQuery.data : [];
    const vocabulary = searchActive
        ? allVocabulary.filter((v) => v.word.toLowerCase().includes(lowerQ))
        : [];

    const isLoading =
        searchActive &&
        (chunksQuery.isLoading ||
            requirementsQuery.isLoading ||
            templatesQuery.isLoading ||
            (codebaseId && vocabularyQuery.isLoading));

    const totalResults = chunks.length + requirements.length + templates.length + vocabulary.length;
    const hasResults = totalResults > 0;

    return (
        <div className="container mx-auto max-w-3xl px-4 py-8">
            {/* Search input */}
            <div className="relative mb-8">
                <SearchIcon className="text-muted-foreground absolute top-1/2 left-4 size-5 -translate-y-1/2" />
                <input
                    ref={inputRef}
                    type="text"
                    value={localQuery}
                    onChange={(e) => handleInputChange(e.target.value)}
                    placeholder="Search across chunks, requirements, templates, and vocabulary..."
                    className="bg-background focus:ring-ring w-full rounded-xl border py-3 pl-12 pr-4 text-base focus:ring-2 focus:outline-none"
                />
            </div>

            {/* Empty state (before searching) */}
            {!searchActive && (
                <div className="flex flex-col items-center gap-3 py-16 text-center">
                    <SearchIcon className="text-muted-foreground/30 size-12" />
                    <p className="text-muted-foreground text-sm">
                        Search across chunks, requirements, templates, and vocabulary
                    </p>
                </div>
            )}

            {/* Loading */}
            {isLoading && (
                <p className="text-muted-foreground py-8 text-center text-sm">Searching...</p>
            )}

            {/* No results */}
            {searchActive && !isLoading && !hasResults && (
                <p className="text-muted-foreground py-8 text-center text-sm">
                    No results for &ldquo;{debouncedQuery}&rdquo;
                </p>
            )}

            {/* Results */}
            {searchActive && !isLoading && hasResults && (
                <div className="space-y-8">
                    {/* Chunks */}
                    {chunks.length > 0 && (
                        <section>
                            <h2 className="mb-3 flex items-center gap-2 text-sm font-semibold">
                                <Blocks className="size-4" />
                                Chunks ({chunks.length})
                            </h2>
                            <div className="space-y-2">
                                {chunks.map((chunk) => {
                                    const id = chunk.id as string;
                                    const title = (chunk.title as string) ?? `Chunk ${id.slice(0, 8)}`;
                                    const type = chunk.type as string;
                                    const content = chunk.content as string;
                                    const updatedAt = chunk.updatedAt as string;
                                    const firstLine = content?.split("\n")[0]?.slice(0, 120) ?? "";

                                    return (
                                        <Link
                                            key={id}
                                            to="/chunks/$chunkId"
                                            params={{ chunkId: id }}
                                            className="hover:bg-muted/50 block rounded-lg border px-4 py-3 transition-colors"
                                        >
                                            <div className="flex items-center gap-2">
                                                <span className="font-medium">{title}</span>
                                                <Badge variant="secondary" size="sm" className="font-mono text-[10px]">
                                                    {type}
                                                </Badge>
                                            </div>
                                            {firstLine && (
                                                <p className="text-muted-foreground mt-1 truncate text-sm">
                                                    {firstLine}
                                                </p>
                                            )}
                                            {updatedAt && (
                                                <p className="text-muted-foreground mt-1 text-xs">
                                                    {timeAgo(updatedAt)}
                                                </p>
                                            )}
                                        </Link>
                                    );
                                })}
                            </div>
                        </section>
                    )}

                    {/* Requirements */}
                    {requirements.length > 0 && (
                        <section>
                            <h2 className="mb-3 flex items-center gap-2 text-sm font-semibold">
                                <ClipboardCheck className="size-4" />
                                Requirements ({requirements.length})
                            </h2>
                            <div className="space-y-2">
                                {requirements.map((req) => {
                                    const id = req.id as string;
                                    const title = (req.title as string) ?? "Untitled";
                                    const status = (req.status as string) ?? "untested";
                                    const priority = req.priority as string | null;
                                    const steps = req.steps as Array<unknown> | undefined;

                                    return (
                                        <Link
                                            key={id}
                                            to="/requirements/$requirementId"
                                            params={{ requirementId: id }}
                                            className="hover:bg-muted/50 block rounded-lg border px-4 py-3 transition-colors"
                                        >
                                            <div className="flex items-center gap-2">
                                                <span className="font-medium">{title}</span>
                                                <Badge
                                                    variant="outline"
                                                    size="sm"
                                                    className={
                                                        status === "passing"
                                                            ? "text-green-600 bg-green-500/10 border-green-500/30"
                                                            : status === "failing"
                                                              ? "text-red-600 bg-red-500/10 border-red-500/30"
                                                              : "text-muted-foreground bg-muted"
                                                    }
                                                >
                                                    {status}
                                                </Badge>
                                                {priority && (
                                                    <Badge variant="secondary" size="sm">
                                                        {priority}
                                                    </Badge>
                                                )}
                                            </div>
                                            <p className="text-muted-foreground mt-1 text-xs">
                                                {steps?.length ?? 0} steps
                                            </p>
                                        </Link>
                                    );
                                })}
                            </div>
                        </section>
                    )}

                    {/* Templates */}
                    {templates.length > 0 && (
                        <section>
                            <h2 className="mb-3 flex items-center gap-2 text-sm font-semibold">
                                <FileText className="size-4" />
                                Templates ({templates.length})
                            </h2>
                            <div className="space-y-2">
                                {templates.map((t) => (
                                    <div
                                        key={t.id}
                                        className="rounded-lg border px-4 py-3"
                                    >
                                        <div className="flex items-center gap-2">
                                            <span className="font-medium">{t.name}</span>
                                            <Badge variant="secondary" size="sm" className="text-[10px]">
                                                {t.type}
                                            </Badge>
                                        </div>
                                        {t.description && (
                                            <p className="text-muted-foreground mt-1 text-sm">
                                                {t.description}
                                            </p>
                                        )}
                                    </div>
                                ))}
                            </div>
                        </section>
                    )}

                    {/* Vocabulary */}
                    {vocabulary.length > 0 && (
                        <section>
                            <h2 className="mb-3 flex items-center gap-2 text-sm font-semibold">
                                <BookOpen className="size-4" />
                                Vocabulary ({vocabulary.length})
                            </h2>
                            <div className="flex flex-wrap gap-2">
                                {vocabulary.map((v) => (
                                    <div
                                        key={v.id}
                                        className="flex items-center gap-2 rounded-lg border px-3 py-1.5"
                                    >
                                        <span className="text-sm font-medium">{v.word}</span>
                                        <Badge variant="outline" size="sm" className="text-[10px]">
                                            {v.category}
                                        </Badge>
                                    </div>
                                ))}
                            </div>
                        </section>
                    )}
                </div>
            )}
        </div>
    );
}
