import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { Network, Save, Search as SearchIcon, Trash2, X } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";

import { Button } from "@/components/ui/button";
import {
    DropdownMenu,
    DropdownMenuContent,
    DropdownMenuItem,
    DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { useActiveCodebase } from "@/features/codebases/use-active-codebase";
import { AddFilterDropdown } from "@/features/search/add-filter-dropdown";
import { FilterPills } from "@/features/search/filter-pills";
import { QueryInput } from "@/features/search/query-input";
import { SearchResults } from "@/features/search/search-results";
import { useQueryBuilder } from "@/features/search/use-query-builder";
import { getUser } from "@/functions/get-user";
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

/** Build a simple query string representation from clauses. */
function clausesToQueryString(clauses: Array<{ field: string; operator: string; value: string; negate?: boolean }>): string {
    return clauses
        .map(c => `${c.negate ? "NOT " : ""}${c.field}:${c.value}`)
        .join(" ");
}

function SearchPage() {
    const { q } = Route.useSearch();
    const navigate = useNavigate();
    const queryClient = useQueryClient();
    const { codebaseId } = useActiveCodebase();

    const builder = useQueryBuilder();
    const [rawInput, setRawInput] = useState(q ?? "");
    const initialLoadDone = useRef(false);
    const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

    // On mount: if `q` param exists, parse it into clauses
    useEffect(() => {
        if (initialLoadDone.current) return;
        initialLoadDone.current = true;
        if (!q) return;

        void (async () => {
            try {
                const result = unwrapEden(
                    await api.api.search.parse.get({ query: { q } })
                );
                if (result && Array.isArray((result as any).clauses)) {
                    builder.loadClauses((result as any).clauses);
                }
            } catch {
                // ignore parse errors — leave clauses empty
            }
        })();
    // eslint-disable-next-line react-hooks/exhaustive-deps
    }, []);

    // Search mutation
    const searchMutation = useMutation({
        mutationFn: async (params: {
            clauses: typeof builder.clauses;
            join: "and" | "or";
            sort: typeof builder.sort;
        }) => {
            return unwrapEden(
                await api.api.search.query.post({
                    clauses: params.clauses,
                    join: params.join,
                    sort: params.sort,
                    limit: 20,
                    offset: 0,
                    codebaseId: codebaseId ?? undefined,
                })
            );
        },
    });

    // Trigger search whenever clauses change (debounced)
    useEffect(() => {
        if (debounceRef.current) clearTimeout(debounceRef.current);
        debounceRef.current = setTimeout(() => {
            if (builder.clauses.length === 0) return;
            searchMutation.mutate({
                clauses: builder.clauses,
                join: builder.join,
                sort: builder.sort,
            });
            // Sync URL
            const qs = clausesToQueryString(builder.clauses);
            void navigate({
                to: "/search",
                search: qs ? { q: qs } : { q: undefined },
                replace: true,
            } as any);
        }, 300);
        return () => {
            if (debounceRef.current) clearTimeout(debounceRef.current);
        };
    // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [builder.clauses, builder.join, builder.sort, codebaseId]);

    // Saved queries
    const savedQueriesQuery = useQuery({
        queryKey: ["search-saved", codebaseId],
        queryFn: async () => {
            return unwrapEden(
                await api.api.search.saved.get({
                    query: { codebaseId: codebaseId ?? undefined },
                })
            );
        },
    });

    const savedQueries = Array.isArray(savedQueriesQuery.data) ? savedQueriesQuery.data : [];

    // Handle raw query input submit (parse & load clauses)
    const handleQuerySubmit = useCallback(
        async (value: string) => {
            if (!value.trim()) return;
            try {
                const result = unwrapEden(
                    await api.api.search.parse.get({ query: { q: value } })
                );
                if (result && Array.isArray((result as any).clauses) && (result as any).clauses.length > 0) {
                    builder.loadClauses((result as any).clauses);
                }
            } catch {
                // ignore
            }
        },
        [builder]
    );

    // Load a saved query
    function handleLoadSavedQuery(saved: any) {
        if (saved?.query?.clauses) {
            builder.loadClauses(saved.query.clauses);
            if (saved.query.join) builder.setJoin(saved.query.join);
            if (saved.query.sort) builder.setSort(saved.query.sort);
        }
    }

    // Save current query
    async function handleSaveQuery() {
        const name = window.prompt("Save query as:");
        if (!name || !name.trim()) return;
        try {
            await unwrapEden(
                await api.api.search.saved.post({
                    name: name.trim(),
                    query: {
                        clauses: builder.clauses,
                        join: builder.join,
                        sort: builder.sort,
                    },
                    codebaseId: codebaseId ?? undefined,
                })
            );
            void queryClient.invalidateQueries({ queryKey: ["search-saved"] });
        } catch {
            // ignore
        }
    }

    // Delete a saved query
    async function handleDeleteSavedQuery(id: string) {
        try {
            await unwrapEden(
                await (api.api.search.saved as any)[id].delete()
            );
            void queryClient.invalidateQueries({ queryKey: ["search-saved"] });
        } catch {
            // ignore
        }
    }

    const results = searchMutation.data as any;
    const chunks = Array.isArray(results?.chunks) ? results.chunks : [];
    const total = typeof results?.total === "number" ? results.total : chunks.length;
    const graphMeta = results?.graphMeta;

    const hasSearched = searchMutation.isSuccess || searchMutation.isPending;

    return (
        <div className="container mx-auto max-w-4xl px-4 py-8">
            {/* Page Header */}
            <div className="mb-6 flex items-center justify-between gap-4">
                <div className="flex items-center gap-3">
                    <h1 className="text-xl font-semibold">Search</h1>

                    {/* Saved queries dropdown */}
                    {savedQueries.length > 0 && (
                        <DropdownMenu>
                            <DropdownMenuTrigger>
                                <Button variant="outline" size="sm" className="gap-1.5 text-xs">
                                    Saved queries
                                </Button>
                            </DropdownMenuTrigger>
                            <DropdownMenuContent align="start" className="w-56">
                                {savedQueries.map((saved: any) => (
                                    <DropdownMenuItem
                                        key={saved.id}
                                        onClick={() => handleLoadSavedQuery(saved)}
                                        className="flex items-center justify-between"
                                    >
                                        <span className="truncate">{saved.name}</span>
                                        <button
                                            onClick={(e) => {
                                                e.stopPropagation();
                                                void handleDeleteSavedQuery(saved.id);
                                            }}
                                            className="ml-2 shrink-0 opacity-40 hover:opacity-100"
                                            aria-label="Delete saved query"
                                        >
                                            <Trash2 className="size-3.5" />
                                        </button>
                                    </DropdownMenuItem>
                                ))}
                            </DropdownMenuContent>
                        </DropdownMenu>
                    )}
                </div>

                <div className="flex items-center gap-2">
                    {/* Save query button */}
                    {builder.clauses.length > 0 && (
                        <Button variant="outline" size="sm" onClick={() => void handleSaveQuery()} className="gap-1.5">
                            <Save className="size-3.5" />
                            Save
                        </Button>
                    )}
                    {/* Clear button */}
                    {builder.clauses.length > 0 && (
                        <Button
                            variant="ghost"
                            size="sm"
                            onClick={() => {
                                builder.clearAll();
                                setRawInput("");
                                void navigate({
                                    to: "/search",
                                    search: { q: undefined },
                                    replace: true,
                                } as any);
                            }}
                            className="gap-1.5"
                        >
                            <X className="size-3.5" />
                            Clear
                        </Button>
                    )}
                </div>
            </div>

            {/* Query Input */}
            <div className="mb-3">
                <QueryInput
                    value={rawInput}
                    onChange={setRawInput}
                    onSubmit={(val) => void handleQuerySubmit(val)}
                />
            </div>

            {/* Filter pills + AND/OR + Add filter */}
            <div className="mb-4 flex flex-wrap items-center gap-2">
                <FilterPills
                    clauses={builder.clauses}
                    join={builder.join}
                    onRemove={builder.removeClause}
                    onSetJoin={builder.setJoin}
                />
                <AddFilterDropdown onAddClause={builder.addClause} />
            </div>

            {/* Graph indicator */}
            {builder.hasGraphClauses && (
                <div className="mb-4 flex items-center gap-2 rounded-lg border border-indigo-500/10 bg-indigo-500/5 px-4 py-2 text-xs text-muted-foreground">
                    <Network className="size-3.5" />
                    <span className="font-semibold uppercase tracking-wider">Graph query active</span>
                </div>
            )}

            {/* Empty state (before any search) */}
            {!hasSearched && builder.clauses.length === 0 && (
                <div className="flex flex-col items-center gap-3 py-16 text-center">
                    <SearchIcon className="size-12 text-muted-foreground/30" />
                    <p className="text-sm text-muted-foreground">
                        Add filters above or type a query and press Enter
                    </p>
                </div>
            )}

            {/* Search results */}
            {(hasSearched || builder.clauses.length > 0) && (
                <SearchResults
                    chunks={chunks}
                    total={total}
                    graphMeta={graphMeta}
                    isLoading={searchMutation.isPending}
                />
            )}
        </div>
    );
}
