import { useMutation, useQuery } from "@tanstack/react-query";
import { useLocation, useNavigate, useSearch } from "@tanstack/react-router";
import { Search, X } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";

import { FILTER_COLORS } from "@/features/search/query-types";
import type { QueryClause } from "@/features/search/query-types";
import { useRecentQueries } from "@/hooks/use-recent-queries";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

import { HeaderSearchDropdown, type SuggestionKind } from "./header-search-dropdown";
import { useHeaderSearchSuggestions } from "./use-header-search-suggestions";

const SLATE_COLOR = "bg-slate-500/15 border-slate-500/30 text-slate-400";

function clausesToQueryString(clauses: QueryClause[]): string {
    return clauses
        .map(c => {
            const prefix = c.negate ? "NOT " : "";
            const value = c.value.includes(" ") ? `"${c.value}"` : c.value;
            if (c.field === "text") return `${prefix}${value}`;
            return `${prefix}${c.field}:${value}`;
        })
        .join(" ");
}

export function HeaderSearchBar() {
    const navigate = useNavigate();
    const location = useLocation();
    const inputRef = useRef<HTMLInputElement>(null);
    const containerRef = useRef<HTMLDivElement>(null);

    const [clauses, setClauses] = useState<QueryClause[]>([]);
    const [rawInput, setRawInput] = useState("");
    const [focused, setFocused] = useState(false);
    const [selectedIdx, setSelectedIdx] = useState(0);
    const [inlineError, setInlineError] = useState<string | null>(null);

    const { items: recentQueries, addQuery: addRecentQuery } = useRecentQueries();

    // Use strict: false so this works from any route
    const searchParams = useSearch({ strict: false }) as { q?: string };

    const savedQueriesQuery = useQuery({
        queryKey: ["saved-queries"],
        queryFn: async () => {
            try {
                const result = unwrapEden(await api.api.search.saved.get());
                return (result as any) ?? [];
            } catch {
                return [];
            }
        },
        staleTime: 5 * 60 * 1000,
    });

    const savedQueries = (savedQueriesQuery.data as Array<{ id: string; name: string; query: unknown }>) ?? [];

    const { suggestions, mode, field } = useHeaderSearchSuggestions(rawInput, clauses.length > 0, savedQueries, recentQueries);

    const parseMutation = useMutation({
        mutationFn: async (q: string) => {
            const result = unwrapEden(await api.api.search.parse.get({ query: { q } as any }));
            return ((result as any)?.clauses ?? []) as QueryClause[];
        },
    });

    // Sync with URL when on /search page
    useEffect(() => {
        if (location.pathname === "/search") {
            const q = searchParams.q;
            if (q) {
                parseMutation.mutateAsync(q).then(setClauses).catch(() => {});
            } else {
                setClauses([]);
            }
        } else {
            setClauses([]);
            setRawInput("");
        }
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [location.pathname, searchParams.q]);

    // Global "/" shortcut to focus
    useEffect(() => {
        function handleKey(e: KeyboardEvent) {
            const tag = document.activeElement?.tagName;
            if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return;
            if (e.key === "/") {
                e.preventDefault();
                inputRef.current?.focus();
            }
        }
        document.addEventListener("keydown", handleKey);
        return () => document.removeEventListener("keydown", handleKey);
    }, []);

    // Click outside to blur
    useEffect(() => {
        function handleClick(e: MouseEvent) {
            if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
                setFocused(false);
            }
        }
        document.addEventListener("mousedown", handleClick);
        return () => document.removeEventListener("mousedown", handleClick);
    }, []);

    const removeClause = useCallback((idx: number) => {
        setClauses(prev => prev.filter((_, i) => i !== idx));
    }, []);

    const submit = useCallback(
        async (explicitClauses?: QueryClause[]) => {
            let finalClauses = explicitClauses ?? clauses;

            if (rawInput.trim() && !explicitClauses) {
                try {
                    const parsed = await parseMutation.mutateAsync(rawInput.trim());
                    if (parsed.length === 0) {
                        setInlineError("Incomplete filter — pick a value");
                        setTimeout(() => setInlineError(null), 3000);
                        return;
                    }
                    finalClauses = [...clauses, ...parsed];
                } catch {
                    setInlineError("Failed to parse query");
                    setTimeout(() => setInlineError(null), 3000);
                    return;
                }
            }

            if (finalClauses.length === 0) return;

            const qs = clausesToQueryString(finalClauses);
            addRecentQuery(qs);
            setRawInput("");
            setClauses(finalClauses);
            setFocused(false);
            inputRef.current?.blur();

            void navigate({ to: "/search", search: { q: qs } as any });
        },
        [clauses, rawInput, parseMutation, addRecentQuery, navigate],
    );

    const handleSuggestionSelect = useCallback(
        async (suggestion: SuggestionKind) => {
            switch (suggestion.type) {
                case "saved":
                    setClauses(suggestion.clauses);
                    setRawInput("");
                    void submit(suggestion.clauses);
                    break;
                case "recent":
                    try {
                        const parsed = await parseMutation.mutateAsync(suggestion.q);
                        setClauses([]);
                        setRawInput("");
                        void submit(parsed);
                    } catch {
                        // ignore
                    }
                    break;
                case "field":
                    setRawInput(() => {
                        const parts = rawInput.split(" ");
                        parts[parts.length - 1] = `${suggestion.field}:`;
                        return parts.join(" ");
                    });
                    inputRef.current?.focus();
                    break;
                case "value": {
                    const newClause: QueryClause = {
                        field: suggestion.field,
                        operator: "is",
                        value: suggestion.value,
                    };
                    setClauses(prev => [...prev, newClause]);
                    setRawInput(prev => {
                        const parts = prev.split(" ");
                        parts.pop();
                        return parts.join(" ") + (parts.length > 0 ? " " : "");
                    });
                    inputRef.current?.focus();
                    break;
                }
                case "chunk":
                    void navigate({ to: "/chunks/$chunkId", params: { chunkId: suggestion.id } });
                    setFocused(false);
                    setRawInput("");
                    break;
                case "text-search":
                    void submit([...clauses, { field: "text", operator: "contains", value: suggestion.q }]);
                    break;
            }
            setSelectedIdx(0);
        },
        [rawInput, clauses, navigate, submit, parseMutation],
    );

    const dropdownOpen = focused && (clauses.length > 0 || rawInput.length > 0 || savedQueries.length > 0 || recentQueries.length > 0);

    const handleKeyDown = useCallback(
        (e: React.KeyboardEvent<HTMLInputElement>) => {
            if (e.key === "Enter") {
                e.preventDefault();
                if (dropdownOpen && suggestions[selectedIdx]) {
                    void handleSuggestionSelect(suggestions[selectedIdx]);
                } else {
                    void submit();
                }
                return;
            }
            if (e.key === "Escape") {
                e.preventDefault();
                setFocused(false);
                inputRef.current?.blur();
                return;
            }
            if (e.key === "Backspace" && rawInput.length === 0 && clauses.length > 0) {
                e.preventDefault();
                setClauses(prev => prev.slice(0, -1));
                return;
            }
            if (e.key === "ArrowDown") {
                e.preventDefault();
                setSelectedIdx(i => Math.min(suggestions.length - 1, i + 1));
                return;
            }
            if (e.key === "ArrowUp") {
                e.preventDefault();
                setSelectedIdx(i => Math.max(0, i - 1));
                return;
            }
        },
        [clauses.length, rawInput.length, submit, dropdownOpen, suggestions, selectedIdx, handleSuggestionSelect],
    );

    return (
        <div className="relative hidden md:block flex-1 max-w-[460px] min-w-[240px]" ref={containerRef}>
            <div
                className={`flex h-9 items-center gap-1.5 rounded-md border px-2 transition-colors ${focused ? "ring-1 ring-ring bg-background" : "bg-muted/40 border-border/50"}`}
                onClick={() => inputRef.current?.focus()}
            >
                <Search className="size-3.5 shrink-0 text-muted-foreground" />

                {clauses.length > 0 && (
                    <div className="flex items-center gap-1 overflow-x-auto max-w-[60%]">
                        {clauses.map((clause, idx) => (
                            <PillChip
                                key={`${clause.field}-${idx}`}
                                clause={clause}
                                onRemove={() => removeClause(idx)}
                            />
                        ))}
                    </div>
                )}

                <input
                    ref={inputRef}
                    type="text"
                    value={rawInput}
                    onChange={e => { setRawInput(e.target.value); setSelectedIdx(0); }}
                    onKeyDown={handleKeyDown}
                    onFocus={() => setFocused(true)}
                    placeholder={clauses.length === 0 ? "Search…" : ""}
                    className="flex-1 min-w-[80px] bg-transparent text-xs font-mono outline-none placeholder:text-muted-foreground"
                    spellCheck={false}
                    autoComplete="off"
                />

                {!focused && (
                    <kbd className="ml-auto shrink-0 rounded border border-border/40 px-1 text-[9px] font-mono text-muted-foreground">
                        /
                    </kbd>
                )}
            </div>

            <HeaderSearchDropdown
                open={dropdownOpen}
                mode={mode}
                field={field}
                suggestions={suggestions}
                savedQueries={savedQueries}
                recentQueries={recentQueries}
                selectedIdx={selectedIdx}
                onSelect={handleSuggestionSelect}
                onHover={setSelectedIdx}
            />

            {inlineError && (
                <div className="absolute left-0 right-0 top-full mt-1 rounded border border-red-500/30 bg-red-500/10 px-3 py-1 text-[10px] text-red-500">
                    {inlineError}
                </div>
            )}
        </div>
    );
}

function PillChip({ clause, onRemove }: { clause: QueryClause; onRemove: () => void }) {
    const colorClass = FILTER_COLORS[clause.field] ?? SLATE_COLOR;
    return (
        <span className={`inline-flex shrink-0 items-center gap-1 rounded border px-1.5 py-0.5 text-[10px] font-mono font-semibold ${colorClass}`}>
            {clause.negate && <span>NOT</span>}
            <span>{clause.field}:{clause.value}</span>
            <button
                type="button"
                onMouseDown={e => { e.preventDefault(); onRemove(); }}
                className="opacity-50 hover:opacity-100 transition-opacity"
                aria-label={`Remove ${clause.field} filter`}
            >
                <X className="size-2.5" />
            </button>
        </span>
    );
}
