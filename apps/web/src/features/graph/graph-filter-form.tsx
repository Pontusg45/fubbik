import { useQuery } from "@tanstack/react-query";
import { ChevronDown, ChevronRight, Compass, Eye, Loader2, Search, X } from "lucide-react";
import { useMemo, useState } from "react";

import { Badge } from "@/components/ui/badge";
import { applyPrefilter } from "@/features/graph/apply-prefilter";
import { useChunkTypes, type ChunkTypeMeta } from "@/features/vocabularies/use-vocabularies";
import { useDebouncedValue } from "@/hooks/use-debounced-value";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

const GROUP_BY = ["tag", "type", "codebase", "none"] as const;
const DEPTHS = [1, 2, 3] as const;

export interface GraphFilterValues {
    tags: string[];
    types: string[];
    focusChunkId: string | null;
    depth: number;
    groupBy: "tag" | "type" | "codebase" | "none";
    tagTypeId: string | null;
    subGroupBy: "type" | "status" | "origin" | null;
}

export const EMPTY_FILTER: GraphFilterValues = {
    tags: [],
    types: [],
    focusChunkId: null,
    depth: 2,
    groupBy: "tag",
    tagTypeId: null,
    subGroupBy: null,
};

export interface GraphFilterPreviewData {
    chunks: Array<{ id: string; type: string; title: string }>;
    connections: Array<{ sourceId: string; targetId: string; relation: string }>;
    chunkTags: Array<{ chunkId: string; tagName: string; tagTypeId?: string | null }>;
}

interface GraphFilterFormProps {
    values: GraphFilterValues;
    onChange: (next: GraphFilterValues) => void;
    /** When provided, render a live preview count. */
    previewData?: GraphFilterPreviewData;
    /** Compact mode shrinks chips and caps visible tag list, for inline panels. */
    compact?: boolean;
    /** Initial state of the focus-chunk title so editing a persisted focus doesn't
     *  require a round-trip to re-resolve its label. */
    initialFocusTitle?: string;
    /** When set, only show tag types that have tags in the current graph data. */
    availableTagTypeIds?: Set<string>;
}

/**
 * Filter fields used in both the pre-render dialog and the top-left inline panel.
 *
 * State is lifted — the form calls `onChange` on every edit so consumers decide
 * when to commit (the dialog buffers until Apply; the top-left panel commits
 * immediately to URL params).
 */
export function GraphFilterForm({ values, onChange, previewData, compact, initialFocusTitle, availableTagTypeIds }: GraphFilterFormProps) {
    const { tags, types, focusChunkId, depth, groupBy, tagTypeId, subGroupBy } = values;
    const [focusChunkTitle, setFocusChunkTitle] = useState(initialFocusTitle ?? "");
    const [search, setSearch] = useState("");
    const [previewExpanded, setPreviewExpanded] = useState(false);

    const tagsQuery = useQuery({
        queryKey: ["tags-list"],
        queryFn: async () => unwrapEden(await api.api.tags.get({ query: {} as never }))
    });

    const tagNames = useMemo(() => {
        const data = tagsQuery.data;
        if (!Array.isArray(data)) return [] as string[];
        return (data as Array<{ name: string }>).map(t => t.name);
    }, [tagsQuery.data]);

    const chunkTypesQuery = useChunkTypes();
    const chunkTypeMetas: ChunkTypeMeta[] = useMemo(() => {
        const loaded = chunkTypesQuery.data;
        if (loaded && loaded.length > 0) return loaded;
        return [] as ChunkTypeMeta[];
    }, [chunkTypesQuery.data]);

    const tagTypesQuery = useQuery({
        queryKey: ["tag-types"],
        queryFn: async () => unwrapEden(await api.api["tag-types"].get()),
        staleTime: 60_000
    });
    const tagTypesList = useMemo(() => {
        const data = tagTypesQuery.data;
        if (!Array.isArray(data)) return [] as Array<{ id: string; name: string; color: string }>;
        const all = data as Array<{ id: string; name: string; color: string }>;
        if (!availableTagTypeIds) return all;
        return all.filter(tt => availableTagTypeIds.has(tt.id));
    }, [tagTypesQuery.data, availableTagTypeIds]);

    const chunkSearchQuery = useQuery({
        queryKey: ["chunks-search-for-focus", search],
        queryFn: async () => unwrapEden(await api.api.chunks.get({ query: { search, limit: "10" } as never })),
        enabled: search.trim().length > 1
    });
    const searchResults = useMemo(() => {
        const data = chunkSearchQuery.data;
        if (!data || typeof data !== "object" || !("chunks" in data)) return [] as Array<{ id: string; title: string; type: string }>;
        return (data as { chunks: Array<{ id: string; title: string; type: string }> }).chunks;
    }, [chunkSearchQuery.data]);

    // Debounce the filter values that feed into the preview: chips toggle
    // instantly (good feedback), but applyPrefilter only runs 120ms after the
    // last change. On medium graphs it's cheap, but linear in chunks + edges,
    // so debouncing keeps the dialog snappy on large KBs.
    const debouncedFilter = useDebouncedValue({ tags, types, focusChunkId, depth }, 120);
    const preview = useMemo(() => {
        if (!previewData) return null;
        const { chunks } = applyPrefilter(previewData, debouncedFilter);
        return {
            totalBefore: previewData.chunks.length,
            totalAfter: chunks.length,
            titles: chunks.map(c => ({ id: c.id, title: c.title, type: c.type }))
        };
    }, [previewData, debouncedFilter]);

    function emit(patch: Partial<GraphFilterValues>) {
        onChange({ ...values, ...patch });
    }

    function toggleTag(t: string) {
        emit({ tags: tags.includes(t) ? tags.filter(x => x !== t) : [...tags, t] });
    }
    function toggleType(t: string) {
        emit({ types: types.includes(t) ? types.filter(x => x !== t) : [...types, t] });
    }

    const tagLimit = compact ? 15 : 30;

    return (
        <div className={compact ? "space-y-3" : "space-y-5"}>
            <FilterSection label="Tags" hint="Show only chunks with these tags (OR match)" compact={compact}>
                {tagsQuery.isLoading ? (
                    <div className="text-muted-foreground flex items-center gap-2 text-xs">
                        <Loader2 className="size-3 animate-spin" /> Loading tags...
                    </div>
                ) : tagNames.length === 0 ? (
                    <p className="text-muted-foreground text-xs">No tags yet.</p>
                ) : (
                    <div className="flex flex-wrap gap-1">
                        {tagNames.slice(0, tagLimit).map(name => {
                            const active = tags.includes(name);
                            return (
                                <button
                                    key={name}
                                    onClick={() => toggleTag(name)}
                                    className={`rounded-full border px-2 py-0.5 text-[10px] ${
                                        active
                                            ? "border-primary bg-primary/10 text-primary"
                                            : "text-muted-foreground hover:text-foreground"
                                    }`}
                                >
                                    {name}
                                </button>
                            );
                        })}
                        {tagNames.length > tagLimit && (
                            <span className="text-muted-foreground px-2 py-0.5 text-[10px]">
                                +{tagNames.length - tagLimit} more
                            </span>
                        )}
                    </div>
                )}
            </FilterSection>

            <FilterSection label="Types" hint="Only include chunks of these types" compact={compact}>
                <div className="flex flex-wrap gap-1">
                    {chunkTypeMetas.map(meta => {
                        const active = types.includes(meta.id);
                        const tooltip = meta.description
                            ? meta.examples.length > 0
                                ? `${meta.description}\nExamples: ${meta.examples.join(", ")}`
                                : meta.description
                            : undefined;
                        return (
                            <button
                                key={meta.id}
                                title={tooltip}
                                onClick={() => toggleType(meta.id)}
                                className={`inline-flex items-center gap-1.5 rounded-full border px-2 py-0.5 text-[10px] ${
                                    active
                                        ? "border-primary bg-primary/10 text-primary"
                                        : "text-muted-foreground hover:text-foreground"
                                }`}
                            >
                                <span aria-hidden className="inline-block size-2 rounded-full" style={{ backgroundColor: meta.color }} />
                                {meta.label}
                            </button>
                        );
                    })}
                </div>
            </FilterSection>

            <FilterSection label="Focus" hint="Keep only chunks within N hops" compact={compact}>
                {focusChunkId ? (
                    <div className="flex flex-wrap items-center gap-1.5">
                        <Badge variant="secondary" className="gap-1">
                            <Compass className="size-3" />
                            <span className="truncate max-w-[140px]">{focusChunkTitle || focusChunkId}</span>
                        </Badge>
                        <button
                            onClick={() => { setFocusChunkTitle(""); emit({ focusChunkId: null }); }}
                            className="text-muted-foreground hover:text-foreground"
                            aria-label="Clear focus"
                        >
                            <X className="size-3.5" />
                        </button>
                        <div className="ml-auto flex items-center gap-1">
                            <span className="text-muted-foreground text-[10px]">Depth</span>
                            {DEPTHS.map(d => (
                                <button
                                    key={d}
                                    onClick={() => emit({ depth: d })}
                                    className={`rounded border px-1.5 py-0.5 text-[10px] ${
                                        depth === d ? "bg-primary text-primary-foreground" : "text-muted-foreground hover:text-foreground"
                                    }`}
                                >
                                    {d}
                                </button>
                            ))}
                        </div>
                    </div>
                ) : (
                    <div className="space-y-1.5">
                        <div className="relative">
                            <Search className="text-muted-foreground absolute left-2 top-1/2 size-3.5 -translate-y-1/2" />
                            <input
                                type="text"
                                value={search}
                                onChange={e => setSearch(e.target.value)}
                                placeholder="Search a chunk..."
                                className="bg-muted/40 w-full rounded-md border py-1.5 pl-7 pr-2 text-[10px] outline-none focus:ring-1 focus:ring-ring"
                            />
                        </div>
                        {searchResults.length > 0 && (
                            <ul className="bg-background max-h-32 overflow-y-auto rounded-md border">
                                {searchResults.map(c => (
                                    <li key={c.id}>
                                        <button
                                            className="hover:bg-muted flex w-full items-center justify-between gap-2 px-2 py-1 text-left text-[10px]"
                                            onClick={() => {
                                                setFocusChunkTitle(c.title);
                                                setSearch("");
                                                emit({ focusChunkId: c.id });
                                            }}
                                        >
                                            <span className="truncate">{c.title}</span>
                                            <span className="text-muted-foreground shrink-0 text-[9px]">{c.type}</span>
                                        </button>
                                    </li>
                                ))}
                            </ul>
                        )}
                    </div>
                )}
            </FilterSection>

            <FilterSection label="Group by" hint="How nodes cluster visually" compact={compact}>
                <div className="flex flex-wrap gap-1">
                    {GROUP_BY.map(g => (
                        <button
                            key={g}
                            onClick={() => emit({ groupBy: g, ...(g !== "tag" ? { tagTypeId: null } : {}) })}
                            className={`rounded-md border px-2 py-0.5 text-[10px] ${
                                groupBy === g ? "bg-primary text-primary-foreground" : "text-muted-foreground hover:text-foreground"
                            }`}
                        >
                            {g === "tag" ? "Tag type" : g === "type" ? "Type" : g === "codebase" ? "Codebase" : "None"}
                        </button>
                    ))}
                </div>
                {groupBy === "tag" && tagTypesList.length > 0 && (
                    <div className="mt-1.5">
                        <div className="flex flex-wrap gap-1">
                            {tagTypesList.map(tt => {
                                const count = previewData
                                    ? new Set(previewData.chunkTags.filter(ct => ct.tagTypeId === tt.id).map(ct => ct.chunkId)).size
                                    : 0;
                                return (
                                    <button
                                        key={tt.id}
                                        onClick={() => emit({ tagTypeId: tagTypeId === tt.id ? null : tt.id })}
                                        className={`inline-flex items-center gap-1 rounded-full border px-2 py-0.5 text-[10px] ${
                                            tagTypeId === tt.id
                                                ? "border-primary bg-primary/10 text-primary"
                                                : "text-muted-foreground hover:text-foreground"
                                        }`}
                                    >
                                        <span className="inline-block size-2 rounded-full" style={{ backgroundColor: tt.color }} />
                                        {tt.name}
                                        {count > 0 && <span className="opacity-60">({count})</span>}
                                    </button>
                                );
                            })}
                        </div>
                        {previewData && (() => {
                            const typedChunks = new Set(previewData.chunkTags.filter(ct => ct.tagTypeId).map(ct => ct.chunkId)).size;
                            const totalChunks = previewData.chunks.length;
                            if (typedChunks < totalChunks) {
                                return (
                                    <p className="text-[10px] text-muted-foreground mt-1.5">
                                        {typedChunks} of {totalChunks} chunks have typed tags. Untyped chunks are hidden unless "Show ungrouped" is on.
                                    </p>
                                );
                            }
                            return null;
                        })()}
                    </div>
                )}
                {groupBy === "tag" && tagTypeId && (
                    <div className="mt-2 border-t pt-2">
                        <div className="text-muted-foreground mb-1 text-[10px]">Then by (sub-group)</div>
                        <div className="flex flex-wrap gap-1">
                            {(["type", "status", "origin"] as const).map(opt => (
                                <button
                                    key={opt}
                                    onClick={() => emit({ subGroupBy: subGroupBy === opt ? null : opt })}
                                    className={`rounded-md border px-2 py-0.5 text-[10px] ${
                                        subGroupBy === opt
                                            ? "bg-primary text-primary-foreground"
                                            : "text-muted-foreground hover:text-foreground"
                                    }`}
                                >
                                    {opt === "type" ? "Type" : opt === "status" ? "Status" : "Origin"}
                                </button>
                            ))}
                        </div>
                    </div>
                )}
            </FilterSection>

            {preview && (
                <div className="rounded-md border bg-muted/30 p-2">
                    <button
                        type="button"
                        onClick={() => setPreviewExpanded(v => !v)}
                        className="flex w-full items-center gap-1.5 text-[10px]"
                    >
                        {previewExpanded ? <ChevronDown className="size-3" /> : <ChevronRight className="size-3" />}
                        <Eye className="text-muted-foreground size-3" />
                        <span className="font-medium">
                            {preview.totalAfter.toLocaleString()}/{preview.totalBefore.toLocaleString()}
                        </span>
                        <span className="text-muted-foreground truncate">
                            {preview.totalAfter === 0 ? "no match" : "chunks"}
                        </span>
                    </button>
                    {previewExpanded && preview.titles.length > 0 && (
                        <ul className="mt-1.5 max-h-32 overflow-y-auto border-t pt-1.5 space-y-0.5">
                            {preview.titles.slice(0, 50).map(t => (
                                <li key={t.id} className="flex items-center gap-1.5 text-[10px]">
                                    <span className="text-muted-foreground shrink-0 font-mono text-[9px] uppercase">{t.type}</span>
                                    <span className="truncate">{t.title}</span>
                                </li>
                            ))}
                            {preview.titles.length > 50 && (
                                <li className="text-muted-foreground mt-0.5 text-[9px] italic">
                                    +{preview.titles.length - 50} more
                                </li>
                            )}
                        </ul>
                    )}
                </div>
            )}
        </div>
    );
}

function FilterSection({
    label,
    hint,
    children,
    compact
}: {
    label: string;
    hint: string;
    children: React.ReactNode;
    compact?: boolean;
}) {
    return (
        <div>
            <div className={compact ? "mb-1" : "mb-1.5"}>
                <div className={compact ? "text-[10px] font-medium uppercase tracking-wide" : "text-xs font-medium uppercase tracking-wide"}>
                    {label}
                </div>
                {!compact && <div className="text-muted-foreground text-[10px]">{hint}</div>}
            </div>
            {children}
        </div>
    );
}
