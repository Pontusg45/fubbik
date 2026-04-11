import { useQuery } from "@tanstack/react-query";
import { useMemo } from "react";

import { FILTER_CATEGORIES } from "@/features/search/query-types";
import type { QueryClause } from "@/features/search/query-types";
import type { RecentQuery } from "@/hooks/use-recent-queries";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

import type { SuggestionKind } from "./header-search-dropdown";

const ENUM_VALUES: Record<string, string[]> = {
    type: ["note", "document", "reference", "schema", "checklist"],
    origin: ["human", "ai"],
    review: ["draft", "reviewed", "approved"],
};

export type HeaderSearchMode = "empty" | "field-prefix" | "value" | "free-text";

export interface HeaderSearchSuggestionsResult {
    mode: HeaderSearchMode;
    field?: string;
    prefix: string;
    suggestions: SuggestionKind[];
}

export function parseMode(rawInput: string): { mode: HeaderSearchMode; field?: string; prefix: string } {
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

export function useHeaderSearchSuggestions(
    rawInput: string,
    hasPills: boolean,
    savedQueries: Array<{ id: string; name: string; query: unknown }>,
    recentQueries: RecentQuery[],
): HeaderSearchSuggestionsResult {
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

    return { mode, field, prefix, suggestions };
}
