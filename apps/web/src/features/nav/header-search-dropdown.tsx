import { useQuery } from "@tanstack/react-query";
import { Clock, FileText, Search, Star } from "lucide-react";
import { useMemo } from "react";

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
 */
function parseMode(rawInput: string): {
    mode: "empty" | "field-prefix" | "value" | "free-text";
    field?: string;
    prefix: string;
} {
    const trimmed = rawInput.trim();
    if (trimmed.length === 0) return { mode: "empty", prefix: "" };

    const lastSpace = trimmed.lastIndexOf(" ");
    const lastToken = lastSpace === -1 ? trimmed : trimmed.slice(lastSpace + 1);
    const colonIdx = lastToken.indexOf(":");

    if (colonIdx === -1) {
        const allFields = FILTER_CATEGORIES.flatMap(c => c.fields.map(f => f.field));
        const matchingFields = allFields.filter(f => f.startsWith(lastToken.toLowerCase()));
        if (matchingFields.length > 0 && lastToken.length >= 1) {
            return { mode: "field-prefix", prefix: lastToken };
        }
        return { mode: "free-text", prefix: lastToken };
    }

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

    const suggestions: SuggestionKind[] = useMemo(() => {
        const list: SuggestionKind[] = [];

        if (mode === "empty" && !hasPills) {
            for (const saved of savedQueries.slice(0, 5)) {
                const query = (saved.query as any) ?? {};
                const clauses = (query.clauses ?? []) as QueryClause[];
                list.push({ type: "saved", name: saved.name, clauses });
            }
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
            if (ENUM_VALUES[field]) {
                const values = ENUM_VALUES[field].filter(v =>
                    v.toLowerCase().startsWith(prefix.toLowerCase()),
                );
                for (const value of values) {
                    list.push({ type: "value", field, value });
                }
                return list;
            }
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
        if (mode === "empty") return null;
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
