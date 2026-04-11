# Header Search Bar Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the 160px plain-text header search input with a ~460px pill-based advanced search bar that has live autocomplete, saved/recent query suggestions, and context-aware Enter behavior.

**Architecture:** Three new frontend files (`header-search-bar.tsx`, `header-search-dropdown.tsx`, `use-recent-queries.ts`). Modify `__root.tsx` to swap in the new component and consolidate primary nav from 7 to 4 links. Reuse existing `/api/search/parse`, `/api/search/autocomplete`, `/api/search/saved` endpoints and `FILTER_COLORS`/`FILTER_CATEGORIES` constants.

**Tech Stack:** React, TanStack Router, TanStack Query, shadcn-ui, Tailwind CSS, Eden API client

---

## File Structure

| File | Action | Responsibility |
|------|--------|---------------|
| `apps/web/src/hooks/use-recent-queries.ts` | Create | Session-storage hook for recent query strings |
| `apps/web/src/features/nav/header-search-dropdown.tsx` | Create | The contextual autocomplete/saved/recent dropdown |
| `apps/web/src/features/nav/header-search-bar.tsx` | Create | The main pill-based search bar component |
| `apps/web/src/routes/__root.tsx` | Modify | Replace old input with new component, consolidate nav links |

---

### Task 1: Create the recent queries hook

**Files:**
- Create: `apps/web/src/hooks/use-recent-queries.ts`

**Context:** Session-storage hook for tracking recent query strings. Pattern identical to `use-recently-viewed.ts` but scoped to sessionStorage (per-browser-session) and stores plain query strings with timestamps.

- [ ] **Step 1: Create the hook file**

```typescript
// apps/web/src/hooks/use-recent-queries.ts
import { useCallback, useEffect, useState } from "react";

const STORAGE_KEY = "fubbik-recent-queries";
const MAX_ITEMS = 15;

export interface RecentQuery {
    q: string;
    usedAt: string;
}

export function useRecentQueries() {
    const [items, setItems] = useState<RecentQuery[]>([]);

    useEffect(() => {
        try {
            const stored = sessionStorage.getItem(STORAGE_KEY);
            if (stored) setItems(JSON.parse(stored));
        } catch {
            // ignore
        }
    }, []);

    const addQuery = useCallback((q: string) => {
        if (!q.trim()) return;
        setItems(prev => {
            const filtered = prev.filter(i => i.q !== q);
            const next = [{ q, usedAt: new Date().toISOString() }, ...filtered].slice(0, MAX_ITEMS);
            try {
                sessionStorage.setItem(STORAGE_KEY, JSON.stringify(next));
            } catch {
                // ignore
            }
            return next;
        });
    }, []);

    const clear = useCallback(() => {
        setItems([]);
        try {
            sessionStorage.removeItem(STORAGE_KEY);
        } catch {
            // ignore
        }
    }, []);

    return { items, addQuery, clear };
}
```

- [ ] **Step 2: Verify type check**

Run: `pnpm --filter web run check-types 2>&1 | grep use-recent-queries`

Expected: No errors.

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/hooks/use-recent-queries.ts
git commit -m "feat(nav): add useRecentQueries hook for sessionStorage query history"
```

---

### Task 2: Create the header search dropdown

**Files:**
- Create: `apps/web/src/features/nav/header-search-dropdown.tsx`

**Context:** The contextual dropdown shown below the search bar. Four states: empty (saved + recent), field prefix, field value, and free text (chunks + search-for-text item).

- [ ] **Step 1: Create the dropdown file**

```typescript
// apps/web/src/features/nav/header-search-dropdown.tsx
import { useQuery } from "@tanstack/react-query";
import { Clock, FileText, Search, Star } from "lucide-react";
import { useEffect, useMemo } from "react";

import { Badge } from "@/components/ui/badge";
import { FILTER_CATEGORIES } from "@/features/search/query-types";
import type { QueryClause } from "@/features/search/query-types";
import type { RecentQuery } from "@/hooks/use-recent-queries";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export type SuggestionKind =
    | { type: "saved"; name: string; clauses: QueryClause[] }
    | { type: "recent"; q: string; usedAt: string }
    | { type: "field"; field: string; label: string; description: string }
    | { type: "value"; field: string; value: string; label?: string }
    | { type: "chunk"; id: string; title: string; chunkType: string }
    | { type: "text-search"; q: string };

export interface HeaderSearchDropdownProps {
    open: boolean;
    rawInput: string;
    hasPills: boolean;
    savedQueries: Array<{ id: string; name: string; query: unknown }>;
    recentQueries: RecentQuery[];
    selectedIdx: number;
    onSelect: (suggestion: SuggestionKind) => void;
    onHover: (idx: number) => void;
}

const ENUM_VALUES: Record<string, string[]> = {
    type: ["note", "document", "reference", "schema", "checklist"],
    origin: ["human", "ai"],
    review: ["draft", "reviewed", "approved"],
};

/**
 * Parse the raw input to determine what kind of suggestions to show.
 * Returns null for empty input (show saved+recent).
 */
function parseMode(rawInput: string): {
    mode: "empty" | "field-prefix" | "value" | "free-text";
    field?: string;
    prefix: string;
} {
    const trimmed = rawInput.trim();
    if (trimmed.length === 0) return { mode: "empty", prefix: "" };

    // Find the last "word" (not yet completed)
    const lastSpace = trimmed.lastIndexOf(" ");
    const lastToken = lastSpace === -1 ? trimmed : trimmed.slice(lastSpace + 1);
    const colonIdx = lastToken.indexOf(":");

    if (colonIdx === -1) {
        // Could be a field prefix like "tag" or free text like "auth flow"
        const allFields = FILTER_CATEGORIES.flatMap(c => c.fields.map(f => f.field));
        const matchingFields = allFields.filter(f => f.startsWith(lastToken.toLowerCase()));
        if (matchingFields.length > 0 && lastToken.length >= 1) {
            return { mode: "field-prefix", prefix: lastToken };
        }
        return { mode: "free-text", prefix: lastToken };
    }

    // Has colon — value completion mode
    const field = lastToken.slice(0, colonIdx);
    const valuePrefix = lastToken.slice(colonIdx + 1);
    return { mode: "value", field, prefix: valuePrefix };
}

export function HeaderSearchDropdown({
    open,
    rawInput,
    hasPills,
    savedQueries,
    recentQueries,
    selectedIdx,
    onSelect,
    onHover,
}: HeaderSearchDropdownProps) {
    const { mode, field, prefix } = parseMode(rawInput);

    // Value completions for dynamic fields
    const valueAutocomplete = useQuery({
        queryKey: ["header-search-autocomplete", field, prefix],
        queryFn: async () => {
            if (!field) return [];
            const acField = field === "near" || field === "path" || field === "similar-to"
                ? "chunk"
                : field === "affected-by"
                  ? "requirement"
                  : field === "tag"
                    ? "tag"
                    : null;
            if (!acField) return [];
            try {
                const result = unwrapEden(
                    await api.api.search.autocomplete.get({
                        query: { field: acField, prefix } as any,
                    }),
                );
                return (result as any) ?? [];
            } catch {
                return [];
            }
        },
        enabled: mode === "value" && !!field,
        staleTime: 30_000,
    });

    // Free-text chunk search
    const chunkSearch = useQuery({
        queryKey: ["header-search-chunks", prefix],
        queryFn: async () => {
            try {
                const result = unwrapEden(
                    await api.api.search.autocomplete.get({
                        query: { field: "chunk", prefix } as any,
                    }),
                );
                return (result as any) ?? [];
            } catch {
                return [];
            }
        },
        enabled: mode === "free-text" && prefix.length >= 1,
        staleTime: 30_000,
    });

    // Build the suggestion list
    const suggestions: SuggestionKind[] = useMemo(() => {
        const list: SuggestionKind[] = [];

        if (mode === "empty" && !hasPills) {
            // Saved queries first
            for (const saved of savedQueries.slice(0, 5)) {
                const query = (saved.query as any) ?? {};
                const clauses = (query.clauses ?? []) as QueryClause[];
                list.push({ type: "saved", name: saved.name, clauses });
            }
            // Then recent
            for (const recent of recentQueries.slice(0, 5)) {
                list.push({ type: "recent", q: recent.q, usedAt: recent.usedAt });
            }
            return list;
        }

        if (mode === "field-prefix") {
            const allFields = FILTER_CATEGORIES.flatMap(c => c.fields);
            const matches = allFields.filter(f => f.field.startsWith(prefix.toLowerCase())).slice(0, 8);
            for (const f of matches) {
                list.push({ type: "field", field: f.field, label: f.label, description: f.description });
            }
            return list;
        }

        if (mode === "value" && field) {
            // Enum fields
            if (ENUM_VALUES[field]) {
                const values = ENUM_VALUES[field].filter(v =>
                    v.toLowerCase().startsWith(prefix.toLowerCase()),
                );
                for (const value of values) {
                    list.push({ type: "value", field, value });
                }
                return list;
            }
            // Dynamic: tag/chunk/requirement autocomplete
            const results = (valueAutocomplete.data as any[]) ?? [];
            for (const r of results.slice(0, 8)) {
                const name = typeof r === "string" ? r : r.name ?? r.title ?? r.id;
                const id = typeof r === "object" ? r.id : undefined;
                list.push({ type: "value", field, value: id ?? name, label: name });
            }
            return list;
        }

        if (mode === "free-text") {
            const results = (chunkSearch.data as any[]) ?? [];
            for (const r of results.slice(0, 5)) {
                list.push({
                    type: "chunk",
                    id: (typeof r === "object" ? r.id : r) as string,
                    title: (typeof r === "object" ? r.title ?? r.name : r) as string,
                    chunkType: (typeof r === "object" ? r.type : "note") as string,
                });
            }
            if (prefix.trim().length > 0) {
                list.push({ type: "text-search", q: prefix });
            }
            return list;
        }

        return list;
    }, [mode, field, prefix, hasPills, savedQueries, recentQueries, valueAutocomplete.data, chunkSearch.data]);

    if (!open || suggestions.length === 0) return null;

    const sectionHeader = (() => {
        if (mode === "empty") return null; // rendered inline below
        if (mode === "field-prefix") return "Fields";
        if (mode === "value") {
            if (field === "tag") return "Tags";
            if (field === "near" || field === "path" || field === "similar-to") return "Chunks";
            if (field === "affected-by") return "Requirements";
            return "Values";
        }
        if (mode === "free-text") return "Chunks";
        return null;
    })();

    return (
        <div
            className="absolute left-0 right-0 top-full z-50 mt-1 max-h-80 overflow-y-auto rounded-md border bg-card shadow-xl"
            role="listbox"
        >
            {mode === "empty" ? (
                <EmptyStateContent
                    savedQueries={savedQueries}
                    recentQueries={recentQueries}
                    selectedIdx={selectedIdx}
                    onSelect={onSelect}
                    onHover={onHover}
                />
            ) : (
                <>
                    {sectionHeader && (
                        <div className="border-b px-3 py-1.5 text-[9px] font-semibold uppercase tracking-wider text-muted-foreground">
                            {sectionHeader}
                        </div>
                    )}
                    <div className="py-1">
                        {suggestions.map((s, i) => (
                            <SuggestionRow
                                key={suggestionKey(s, i)}
                                suggestion={s}
                                selected={i === selectedIdx}
                                onSelect={() => onSelect(s)}
                                onHover={() => onHover(i)}
                            />
                        ))}
                    </div>
                </>
            )}
            <div className="border-t px-3 py-1 text-[9px] text-muted-foreground flex justify-between font-mono">
                <span>↑↓ navigate · ⏎ select</span>
                <span>⇧⏎ full search</span>
            </div>
        </div>
    );
}

function suggestionKey(s: SuggestionKind, i: number): string {
    switch (s.type) {
        case "saved": return `saved-${s.name}-${i}`;
        case "recent": return `recent-${s.q}-${i}`;
        case "field": return `field-${s.field}`;
        case "value": return `value-${s.field}-${s.value}-${i}`;
        case "chunk": return `chunk-${s.id}`;
        case "text-search": return `text-${s.q}`;
    }
}

function EmptyStateContent({
    savedQueries,
    recentQueries,
    selectedIdx,
    onSelect,
    onHover,
}: {
    savedQueries: Array<{ id: string; name: string; query: unknown }>;
    recentQueries: RecentQuery[];
    selectedIdx: number;
    onSelect: (suggestion: SuggestionKind) => void;
    onHover: (idx: number) => void;
}) {
    const savedToShow = savedQueries.slice(0, 5);
    const recentToShow = recentQueries.slice(0, 5);

    let idx = 0;

    return (
        <>
            {savedToShow.length > 0 && (
                <>
                    <div className="border-b px-3 py-1.5 text-[9px] font-semibold uppercase tracking-wider text-muted-foreground">
                        Saved queries
                    </div>
                    <div className="py-1">
                        {savedToShow.map(saved => {
                            const currentIdx = idx++;
                            const query = (saved.query as any) ?? {};
                            const clauses = (query.clauses ?? []) as QueryClause[];
                            const preview = clauses
                                .map(c => `${c.negate ? "NOT " : ""}${c.field}:${c.value}`)
                                .join(" ");
                            return (
                                <button
                                    key={`saved-${saved.id}`}
                                    type="button"
                                    onMouseDown={e => { e.preventDefault(); onSelect({ type: "saved", name: saved.name, clauses }); }}
                                    onMouseEnter={() => onHover(currentIdx)}
                                    className={`flex w-full items-center gap-2 px-3 py-1.5 text-left text-xs transition-colors ${currentIdx === selectedIdx ? "bg-muted" : "hover:bg-muted/50"}`}
                                >
                                    <Star className="size-3 shrink-0 text-yellow-500/70" />
                                    <span className="shrink-0 truncate">{saved.name}</span>
                                    <span className="ml-auto truncate text-[10px] text-muted-foreground/60 font-mono">
                                        {preview}
                                    </span>
                                </button>
                            );
                        })}
                    </div>
                </>
            )}
            {recentToShow.length > 0 && (
                <>
                    <div className="border-t border-b px-3 py-1.5 text-[9px] font-semibold uppercase tracking-wider text-muted-foreground">
                        Recent
                    </div>
                    <div className="py-1">
                        {recentToShow.map(recent => {
                            const currentIdx = idx++;
                            return (
                                <button
                                    key={`recent-${recent.q}-${recent.usedAt}`}
                                    type="button"
                                    onMouseDown={e => { e.preventDefault(); onSelect({ type: "recent", q: recent.q, usedAt: recent.usedAt }); }}
                                    onMouseEnter={() => onHover(currentIdx)}
                                    className={`flex w-full items-center gap-2 px-3 py-1.5 text-left text-xs transition-colors ${currentIdx === selectedIdx ? "bg-muted" : "hover:bg-muted/50"}`}
                                >
                                    <Clock className="size-3 shrink-0 text-muted-foreground/60" />
                                    <span className="truncate font-mono text-[10px]">{recent.q}</span>
                                </button>
                            );
                        })}
                    </div>
                </>
            )}
            {savedToShow.length === 0 && recentToShow.length === 0 && (
                <div className="px-3 py-4 text-center text-xs text-muted-foreground">
                    Start typing to search or filter
                </div>
            )}
        </>
    );
}

function SuggestionRow({
    suggestion,
    selected,
    onSelect,
    onHover,
}: {
    suggestion: SuggestionKind;
    selected: boolean;
    onSelect: () => void;
    onHover: () => void;
}) {
    const icon = (() => {
        switch (suggestion.type) {
            case "field": return <Search className="size-3 shrink-0 text-muted-foreground/60" />;
            case "value": return <Badge variant="secondary" size="sm" className="shrink-0 font-mono text-[8px]">{suggestion.field}</Badge>;
            case "chunk": return <FileText className="size-3 shrink-0 text-muted-foreground/60" />;
            case "text-search": return <Search className="size-3 shrink-0 text-muted-foreground/60" />;
            default: return null;
        }
    })();

    const label = (() => {
        switch (suggestion.type) {
            case "field": return suggestion.label;
            case "value": return suggestion.label ?? suggestion.value;
            case "chunk": return suggestion.title;
            case "text-search": return `Search for "${suggestion.q}"`;
            default: return "";
        }
    })();

    const rightMeta = (() => {
        switch (suggestion.type) {
            case "field": return suggestion.description;
            case "chunk": return suggestion.chunkType;
            default: return null;
        }
    })();

    return (
        <button
            type="button"
            onMouseDown={e => { e.preventDefault(); onSelect(); }}
            onMouseEnter={onHover}
            className={`flex w-full items-center gap-2 px-3 py-1.5 text-left text-xs transition-colors ${selected ? "bg-muted" : "hover:bg-muted/50"}`}
        >
            {icon}
            <span className="truncate">{label}</span>
            {rightMeta && (
                <span className="ml-auto shrink-0 text-[10px] text-muted-foreground/60 font-mono">
                    {rightMeta}
                </span>
            )}
        </button>
    );
}
```

- [ ] **Step 2: Verify type check**

Run: `pnpm --filter web run check-types 2>&1 | grep header-search-dropdown`

Expected: No errors. If `api.api.search.autocomplete.get` has a different signature, adjust the `query` shape.

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/features/nav/header-search-dropdown.tsx
git commit -m "feat(nav): add HeaderSearchDropdown with contextual suggestions"
```

---

### Task 3: Create the header search bar

**Files:**
- Create: `apps/web/src/features/nav/header-search-bar.tsx`

**Context:** The main pill-based search bar component. Manages clauses, raw input, autocomplete state, and keyboard navigation. Syncs with the `/search` route via URL params.

- [ ] **Step 1: Create the component**

```typescript
// apps/web/src/features/nav/header-search-bar.tsx
import { useMutation, useQuery } from "@tanstack/react-query";
import { useLocation, useNavigate } from "@tanstack/react-router";
import { Search, X } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { FILTER_COLORS } from "@/features/search/query-types";
import type { QueryClause } from "@/features/search/query-types";
import { useRecentQueries } from "@/hooks/use-recent-queries";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

import { HeaderSearchDropdown, type SuggestionKind } from "./header-search-dropdown";

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

    // Saved queries
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

    // Parse raw input to clauses via the API
    const parseMutation = useMutation({
        mutationFn: async (q: string) => {
            const result = unwrapEden(await api.api.search.parse.get({ query: { q } as any }));
            return ((result as any)?.clauses ?? []) as QueryClause[];
        },
    });

    // Sync with URL when on /search page
    useEffect(() => {
        if (location.pathname === "/search") {
            const q = new URLSearchParams(location.search).get("q");
            if (q) {
                parseMutation.mutateAsync(q).then(setClauses).catch(() => {});
            } else {
                setClauses([]);
            }
        } else {
            // Reset when navigating away from /search
            setClauses([]);
            setRawInput("");
        }
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [location.pathname, location.search]);

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

            // If there's raw input, try to parse it into additional clauses
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
                        void submit(parsed);
                    } catch {
                        // ignore
                    }
                    break;
                case "field":
                    // Scaffold "<field>:" in input, keep focus
                    setRawInput(() => {
                        const parts = rawInput.split(" ");
                        parts[parts.length - 1] = `${suggestion.field}:`;
                        return parts.join(" ");
                    });
                    inputRef.current?.focus();
                    break;
                case "value": {
                    // Create the pill, clear input
                    const newClause: QueryClause = {
                        field: suggestion.field,
                        operator: "is",
                        value: suggestion.value,
                    };
                    setClauses(prev => [...prev, newClause]);
                    // Remove the trailing "<field>:<prefix>" from raw input
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

    const handleKeyDown = useCallback(
        (e: React.KeyboardEvent<HTMLInputElement>) => {
            if (e.key === "Enter") {
                e.preventDefault();
                if (e.shiftKey) {
                    void submit();
                    return;
                }
                void submit();
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
                setSelectedIdx(i => i + 1);
                return;
            }
            if (e.key === "ArrowUp") {
                e.preventDefault();
                setSelectedIdx(i => Math.max(0, i - 1));
                return;
            }
        },
        [clauses.length, rawInput.length, submit],
    );

    const dropdownOpen = focused && (clauses.length > 0 || rawInput.length > 0 || savedQueries.length > 0 || recentQueries.length > 0);

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
                rawInput={rawInput}
                hasPills={clauses.length > 0}
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
```

- [ ] **Step 2: Verify type check**

Run: `pnpm --filter web run check-types 2>&1 | grep header-search-bar`

Expected: No errors. If `api.api.search.parse.get` or `api.api.search.saved.get` have different signatures, adapt the calls.

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/features/nav/header-search-bar.tsx
git commit -m "feat(nav): add HeaderSearchBar with pills, autocomplete, and context-aware Enter"
```

---

### Task 4: Wire into root layout

**Files:**
- Modify: `apps/web/src/routes/__root.tsx`

**Context:** Replace the current `<input>`-based search with `<HeaderSearchBar />`. Consolidate primary nav from 7 links to 4. Move Features, Reviews, Docs into the Manage dropdown under a new "Navigate" section.

- [ ] **Step 1: Update imports**

In `apps/web/src/routes/__root.tsx`, at the top:

Remove: `Search` from `lucide-react` import (no longer needed — HeaderSearchBar owns the icon). Add these to the lucide-react import: `Compass, FileText, MessageSquare` (for Navigate menu items).

Actually check the current lucide imports first. `FileText` may already be imported. Add only missing ones.

Add the new import:
```typescript
import { HeaderSearchBar } from "@/features/nav/header-search-bar";
```

Remove these hooks/state:
- `useNavigate` — may still be needed elsewhere in the file
- `const [navSearch, setNavSearch] = useState("");`
- `const searchInputRef = useRef<HTMLInputElement>(null);`
- The `useEffect` that handles the `/` keyboard shortcut

- [ ] **Step 2: Replace the old search input block**

Find this block in the nav:
```tsx
<div className="relative hidden md:block">
    <Search className="text-muted-foreground pointer-events-none absolute left-2.5 top-1/2 size-3.5 -translate-y-1/2" />
    <input
        ref={searchInputRef}
        type="text"
        placeholder="Search... (press /)"
        value={navSearch}
        onChange={e => setNavSearch(e.target.value)}
        onKeyDown={e => {
            if (e.key === "Enter" && navSearch.trim()) {
                navigate({ to: "/search", search: { q: navSearch.trim() } });
                setNavSearch("");
                e.currentTarget.blur();
            }
            if (e.key === "Escape") {
                setNavSearch("");
                e.currentTarget.blur();
            }
        }}
        className="bg-muted/50 border-border/50 text-foreground placeholder:text-muted-foreground h-8 w-40 rounded-md border pl-8 pr-3 text-xs transition-all focus:w-56 focus:outline-none focus:ring-1 focus:ring-ring"
    />
</div>
```

Replace with:
```tsx
<HeaderSearchBar />
```

- [ ] **Step 3: Remove consolidated nav links**

Delete the three `<Link>` elements for Features, Reviews, and Docs from the primary nav.

Find each one and delete. Example:
```tsx
<Link
    to="/features"
    className="text-muted-foreground hover:text-foreground [&.active]:text-foreground rounded-md px-3 py-1.5 text-sm font-medium transition-colors"
>
    Features
</Link>
```

Same for `/reviews` and `/docs`.

- [ ] **Step 4: Add the Navigate section to the Manage dropdown**

Find the existing `<DropdownMenuContent align="start">` inside the Manage dropdown. Add these items at the top, before the existing "Tags" item:

```tsx
<DropdownMenuItem render={<Link to="/features" />}>
    <Compass className="size-4" />
    Features
</DropdownMenuItem>
<DropdownMenuItem render={<Link to="/reviews" />}>
    <MessageSquare className="size-4" />
    Reviews
</DropdownMenuItem>
<DropdownMenuItem render={<Link to="/docs" search={{}} />}>
    <FileText className="size-4" />
    Docs
</DropdownMenuItem>
<DropdownMenuSeparator />
```

Make sure `Compass` and `MessageSquare` are imported from lucide-react (add to existing import). `FileText` may already be imported.

- [ ] **Step 5: Remove the `/` keyboard shortcut useEffect**

The `/` key handler is now inside `HeaderSearchBar`. Remove this block from `__root.tsx`:
```typescript
useEffect(() => {
    function isInputFocused() {
        const tag = document.activeElement?.tagName;
        return tag === "INPUT" || tag === "TEXTAREA" || (document.activeElement as HTMLElement)?.isContentEditable;
    }
    function handleKeyDown(e: KeyboardEvent) {
        if (e.key === "/" && !isInputFocused()) {
            e.preventDefault();
            searchInputRef.current?.focus();
        }
    }
    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
}, []);
```

Also remove `useEffect`, `useRef`, `useState` from the React import if they're no longer used in `__root.tsx`. Keep them if they're still used by other code in the file.

- [ ] **Step 6: Verify type check**

Run: `pnpm --filter web run check-types 2>&1 | grep __root`

Expected: No new errors. If `useNavigate` is no longer used, remove it from the import.

- [ ] **Step 7: Smoke test**

Run: `pnpm dev`

Navigate to any non-landing page and verify:
1. Search bar shows at ~460px width in the nav row
2. Dashboard, Chunks, Graph, Requirements are the only primary nav links
3. Manage dropdown contains Features, Reviews, Docs at the top
4. Press `/` — focus jumps to search bar
5. Type `type:reference` and press Space — dropdown shows "reference" value
6. Select it — pill appears, input clears
7. Press Enter — navigates to `/search?q=type:reference`
8. Focus search bar with empty input — dropdown shows saved queries and recent queries (if any)
9. Press Backspace with empty input — removes last pill
10. Click outside — dropdown closes

- [ ] **Step 8: Commit**

```bash
git add apps/web/src/routes/__root.tsx
git commit -m "feat(nav): wire HeaderSearchBar into root layout, consolidate primary nav"
```

---

### Task 5: Final Verification

**Files:** (verification only)

- [ ] **Step 1: Full type check**

Run: `pnpm --filter web run check-types 2>&1 | head -30`

Expected: No new errors in `header-search-bar`, `header-search-dropdown`, `use-recent-queries`, or `__root.tsx`.

- [ ] **Step 2: Build**

Run: `pnpm build --filter=web`

Expected: Success.

- [ ] **Step 3: Manual checklist**

| Test | Expected |
|------|----------|
| Open any non-landing page | Larger search bar visible, ~460px max |
| Press `/` | Bar focuses |
| Type `type:` | Dropdown shows 5 type values |
| Pick "reference" | Pill forms, input clears |
| Type `tag:` | Dropdown shows live tag autocomplete |
| Type free text "auth" | Dropdown shows matching chunks |
| Click a chunk suggestion | Navigates to chunk detail |
| Focus empty bar | Dropdown shows saved + recent queries |
| Click a saved query | Pills load, navigates to /search |
| Press Backspace with empty input | Last pill removed |
| Press Enter with pills | Navigates to /search?q=... |
| Already on /search, press Enter | URL updates in place, search re-runs |
| Nav has only 4 primary links | Dashboard, Chunks, Graph, Requirements |
| Manage dropdown has Features/Reviews/Docs at top | ✓ |
| Press Escape with focus | Dropdown closes, input blurs |
| Click outside the bar | Dropdown closes |

- [ ] **Step 4: Commit fixes if needed**

If any smoke tests fail, fix and commit:
```bash
git commit -am "fix(nav): resolve header search bar smoke test issues"
```
