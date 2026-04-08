import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { Cable, Clock, Network, Search, Tags, X } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { Separator } from "@/components/ui/separator";
import { SkeletonList } from "@/components/ui/skeleton-list";
import { SearchGraph } from "@/features/search/search-graph";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

interface GraphContext {
    hopDistance?: number;
    pathPosition?: number;
    matchedRequirement?: string;
}

interface ChunkResult {
    id: string;
    title: string;
    type: string;
    summary?: string | null;
    tags?: Array<{ name: string }>;
    connectionCount?: number;
    updatedAt?: string | null;
    graphContext?: GraphContext;
    healthScore?: number;
}

interface GraphMeta {
    type: string;
    referenceChunk?: string;
    pathChunks?: string[];
}

interface SearchResultsProps {
    chunks: ChunkResult[];
    total: number;
    graphMeta?: GraphMeta;
    isLoading: boolean;
}

const TYPE_COLORS: Record<string, string> = {
    note: "bg-blue-500/10 text-blue-400 border-blue-500/20",
    document: "bg-purple-500/10 text-purple-400 border-purple-500/20",
    reference: "bg-green-500/10 text-green-400 border-green-500/20",
    schema: "bg-orange-500/10 text-orange-400 border-orange-500/20",
    checklist: "bg-pink-500/10 text-pink-400 border-pink-500/20",
};

const RELATION_TYPES = [
    "related_to",
    "part_of",
    "depends_on",
    "extends",
    "references",
    "supports",
    "contradicts",
    "alternative_to",
];

function formatRelativeTime(dateStr: string): string {
    const date = new Date(dateStr);
    const now = new Date();
    const diffMs = now.getTime() - date.getTime();
    const diffDays = Math.floor(diffMs / (1000 * 60 * 60 * 24));
    if (diffDays === 0) return "today";
    if (diffDays === 1) return "yesterday";
    if (diffDays < 30) return `${diffDays}d ago`;
    if (diffDays < 365) return `${Math.floor(diffDays / 30)}mo ago`;
    return `${Math.floor(diffDays / 365)}y ago`;
}

// ─── Bulk Action Bar ─────────────────────────────────────────────────────────

interface BulkActionBarProps {
    selectedIds: Set<string>;
    onClear: () => void;
}

function SearchBulkActionBar({ selectedIds, onClear }: BulkActionBarProps) {
    const queryClient = useQueryClient();

    const [showTagInput, setShowTagInput] = useState(false);
    const [tagInput, setTagInput] = useState("");

    const [showReqSearch, setShowReqSearch] = useState(false);
    const [reqSearch, setReqSearch] = useState("");

    const [showConnectPanel, setShowConnectPanel] = useState(false);
    const [connectRelation, setConnectRelation] = useState("related_to");

    const addTagMutation = useMutation({
        mutationFn: async (tags: string) => {
            const { data, error } = await api.api.chunks["bulk-update"].post({
                ids: [...selectedIds],
                action: "add_tags",
                value: tags,
            });
            if (error) throw new Error("Failed to add tags");
            return data;
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["search"] });
            toast.success(`Tags added to ${selectedIds.size} chunk${selectedIds.size !== 1 ? "s" : ""}`);
            setTagInput("");
            setShowTagInput(false);
        },
        onError: () => {
            toast.error("Failed to add tags");
        },
    });

    const reqSearchQuery = useQuery({
        queryKey: ["requirements", "search-bulk", reqSearch],
        queryFn: async () => {
            const result = unwrapEden(
                await api.api.requirements.get({ query: { search: reqSearch, limit: "10" } })
            ) as { requirements?: Array<{ id: string; title: string }> } | null;
            return result?.requirements ?? [];
        },
        enabled: showReqSearch && reqSearch.trim().length > 0,
    });

    const linkRequirementMutation = useMutation({
        mutationFn: async ({ reqId, existingChunkIds }: { reqId: string; existingChunkIds: string[] }) => {
            const merged = Array.from(new Set([...existingChunkIds, ...selectedIds]));
            const { error } = await (api.api.requirements as any)[reqId].chunks.put({ chunkIds: merged });
            if (error) throw new Error("Failed to link requirement");
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["requirements"] });
            toast.success(`Linked ${selectedIds.size} chunk${selectedIds.size !== 1 ? "s" : ""} to requirement`);
            setReqSearch("");
            setShowReqSearch(false);
        },
        onError: () => {
            toast.error("Failed to link to requirement");
        },
    });

    const createConnectionsMutation = useMutation({
        mutationFn: async () => {
            const ids = [...selectedIds];
            for (let i = 0; i < ids.length - 1; i++) {
                const { error } = await api.api.connections.post({
                    sourceId: ids[i]!,
                    targetId: ids[i + 1]!,
                    relation: connectRelation,
                });
                if (error) throw new Error("Failed to create connection");
            }
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["search"] });
            toast.success(`Created ${selectedIds.size - 1} connection${selectedIds.size - 1 !== 1 ? "s" : ""}`);
            setShowConnectPanel(false);
            onClear();
        },
        onError: () => {
            toast.error("Failed to create connections");
        },
    });

    if (selectedIds.size === 0) return null;

    async function handleLinkRequirement(req: { id: string; title: string }) {
        try {
            const result = unwrapEden(
                await (api.api.requirements as any)[req.id].get()
            ) as { chunkIds?: string[] } | null;
            linkRequirementMutation.mutate({ reqId: req.id, existingChunkIds: result?.chunkIds ?? [] });
        } catch {
            linkRequirementMutation.mutate({ reqId: req.id, existingChunkIds: [] });
        }
    }

    const reqResults = reqSearchQuery.data ?? [];

    return (
        <div className="bg-background fixed bottom-6 left-1/2 z-50 flex -translate-x-1/2 items-center gap-3 rounded-lg border px-4 py-2.5 shadow-lg">
            <span className="text-sm font-medium">{selectedIds.size} selected</span>
            <Separator orientation="vertical" className="h-5" />

            {showTagInput ? (
                <div className="flex items-center gap-1.5">
                    <input
                        type="text"
                        value={tagInput}
                        onChange={e => setTagInput(e.target.value)}
                        onKeyDown={e => {
                            if (e.key === "Enter" && tagInput.trim()) {
                                addTagMutation.mutate(tagInput.trim());
                            }
                        }}
                        placeholder="tag1, tag2, ..."
                        className="bg-background w-36 rounded border px-2 py-1 text-xs"
                        autoFocus
                    />
                    <Button
                        size="sm"
                        variant="outline"
                        onClick={() => tagInput.trim() && addTagMutation.mutate(tagInput.trim())}
                        disabled={addTagMutation.isPending}
                    >
                        Add
                    </Button>
                    <Button size="sm" variant="ghost" onClick={() => setShowTagInput(false)}>
                        <X className="size-3" />
                    </Button>
                </div>
            ) : (
                <Button
                    variant="outline"
                    size="sm"
                    onClick={() => {
                        setShowReqSearch(false);
                        setShowConnectPanel(false);
                        setShowTagInput(true);
                    }}
                >
                    <Tags className="size-3.5" />
                    Add Tag
                </Button>
            )}

            <Popover
                open={showReqSearch}
                onOpenChange={open => {
                    setShowReqSearch(open);
                    if (open) { setShowTagInput(false); setShowConnectPanel(false); }
                }}
            >
                <PopoverTrigger render={<Button variant="outline" size="sm" />}>
                    <Cable className="size-3.5" />
                    Link to Requirement
                </PopoverTrigger>
                <PopoverContent side="top" align="center" className="w-72">
                    <div className="space-y-2">
                        <label className="text-muted-foreground block text-xs font-medium">Search requirements</label>
                        <div className="relative">
                            <Search className="text-muted-foreground pointer-events-none absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2" />
                            <input
                                type="text"
                                value={reqSearch}
                                onChange={e => setReqSearch(e.target.value)}
                                placeholder="Requirement title..."
                                className="bg-background w-full rounded-md border py-1.5 pl-8 pr-3 text-sm"
                                autoFocus
                            />
                        </div>
                        {reqResults.length > 0 && (
                            <div className="max-h-48 space-y-0.5 overflow-y-auto rounded-md border p-1">
                                {reqResults.map(req => (
                                    <button
                                        key={req.id}
                                        type="button"
                                        onClick={() => handleLinkRequirement(req)}
                                        disabled={linkRequirementMutation.isPending}
                                        className="hover:bg-muted w-full rounded-md px-2.5 py-1.5 text-left text-sm transition-colors"
                                    >
                                        {req.title}
                                    </button>
                                ))}
                            </div>
                        )}
                        {reqSearch.trim().length > 0 && reqResults.length === 0 && !reqSearchQuery.isLoading && (
                            <p className="text-muted-foreground py-2 text-center text-xs">No requirements found</p>
                        )}
                    </div>
                </PopoverContent>
            </Popover>

            {selectedIds.size >= 2 && (
                <Popover
                    open={showConnectPanel}
                    onOpenChange={open => {
                        setShowConnectPanel(open);
                        if (open) { setShowTagInput(false); setShowReqSearch(false); }
                    }}
                >
                    <PopoverTrigger render={<Button variant="outline" size="sm" />}>
                        <Cable className="size-3.5" />
                        Create Connections
                    </PopoverTrigger>
                    <PopoverContent side="top" align="center" className="w-64">
                        <div className="space-y-3">
                            <p className="text-muted-foreground text-xs">
                                Creates sequential connections between the {selectedIds.size} selected chunks (
                                {selectedIds.size - 1} connection{selectedIds.size - 1 !== 1 ? "s" : ""}).
                            </p>
                            <div>
                                <label className="text-muted-foreground mb-1.5 block text-xs font-medium">Relation type</label>
                                <select
                                    value={connectRelation}
                                    onChange={e => setConnectRelation(e.target.value)}
                                    className="bg-background w-full rounded-md border px-2.5 py-1.5 text-sm"
                                >
                                    {RELATION_TYPES.map(r => (
                                        <option key={r} value={r}>{r.replace(/_/g, " ")}</option>
                                    ))}
                                </select>
                            </div>
                            <Button
                                size="sm"
                                className="w-full"
                                onClick={() => createConnectionsMutation.mutate()}
                                disabled={createConnectionsMutation.isPending}
                            >
                                {createConnectionsMutation.isPending ? "Creating..." : "Create connections"}
                            </Button>
                        </div>
                    </PopoverContent>
                </Popover>
            )}

            <Separator orientation="vertical" className="h-5" />
            <Button variant="ghost" size="sm" onClick={onClear}>
                <X className="size-3.5" />
                Clear
            </Button>
        </div>
    );
}

// ─── Main component ───────────────────────────────────────────────────────────

export function SearchResults({ chunks, total, graphMeta, isLoading }: SearchResultsProps) {
    const [showGraph, setShowGraph] = useState(false);
    const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());

    function toggleId(id: string) {
        setSelectedIds(prev => {
            const next = new Set(prev);
            if (next.has(id)) next.delete(id);
            else next.add(id);
            return next;
        });
    }

    function toggleAll() {
        if (selectedIds.size === chunks.length && chunks.length > 0) {
            setSelectedIds(new Set());
        } else {
            setSelectedIds(new Set(chunks.map(c => c.id)));
        }
    }

    const allSelected = chunks.length > 0 && selectedIds.size === chunks.length;
    const someSelected = selectedIds.size > 0 && selectedIds.size < chunks.length;

    if (isLoading) {
        return <SkeletonList count={6} />;
    }

    if (chunks.length === 0) {
        return (
            <div className="flex flex-col items-center justify-center gap-3 py-16 text-center">
                <Search className="size-10 text-muted-foreground opacity-40" />
                <p className="text-sm font-medium text-muted-foreground">No results found</p>
                <p className="text-xs text-muted-foreground opacity-70">
                    Try adjusting your filters or search query
                </p>
            </div>
        );
    }

    return (
        <div className="space-y-1">
            {/* Result count + graph badge + select-all + toggle */}
            <div className="mb-3 flex items-center gap-2">
                <input
                    type="checkbox"
                    checked={allSelected}
                    ref={el => { if (el) el.indeterminate = someSelected; }}
                    onChange={toggleAll}
                    className="size-3.5 cursor-pointer rounded accent-primary"
                    aria-label="Select all results"
                />
                <span className="text-xs text-muted-foreground">
                    {total} result{total !== 1 ? "s" : ""}
                </span>
                {graphMeta && (
                    <Badge variant="outline" size="sm" className="border-amber-500/30 bg-amber-500/10 text-amber-400">
                        Graph filtered
                        {graphMeta.type === "near" && graphMeta.referenceChunk
                            ? ` · near ${graphMeta.referenceChunk}`
                            : null}
                        {graphMeta.type === "path" && graphMeta.pathChunks?.length
                            ? ` · path (${graphMeta.pathChunks.length} hops)`
                            : null}
                    </Badge>
                )}
                <Button
                    variant={showGraph ? "secondary" : "ghost"}
                    size="sm"
                    onClick={() => setShowGraph(g => !g)}
                    className="ml-auto gap-1.5"
                    title={showGraph ? "Hide graph" : "Show graph"}
                >
                    <Network className="size-3.5" />
                    {showGraph ? "Hide graph" : "Show graph"}
                </Button>
            </div>

            {/* Minimap graph */}
            {showGraph && (
                <div className="mb-4">
                    <SearchGraph
                        chunkIds={chunks.map(c => c.id)}
                        chunks={chunks.map(c => ({ id: c.id, title: c.title, type: c.type }))}
                    />
                </div>
            )}

            {/* Chunk rows */}
            <div className="divide-y divide-border rounded-md border">
                {chunks.map(chunk => {
                    const typeColor = TYPE_COLORS[chunk.type] ?? "bg-slate-500/10 text-slate-400 border-slate-500/20";
                    const isSelected = selectedIds.has(chunk.id);
                    const dotColor = typeColor.includes("blue") ? "#60a5fa"
                        : typeColor.includes("purple") ? "#c084fc"
                        : typeColor.includes("green") ? "#4ade80"
                        : typeColor.includes("orange") ? "#fb923c"
                        : typeColor.includes("pink") ? "#f472b6"
                        : "#94a3b8";
                    return (
                        <div
                            key={chunk.id}
                            className={`flex items-start gap-3 p-3 transition-colors ${isSelected ? "bg-muted/60" : "hover:bg-muted/50"}`}
                        >
                            {/* Checkbox + type dot */}
                            <div className="mt-1 flex shrink-0 items-center gap-2">
                                <input
                                    type="checkbox"
                                    checked={isSelected}
                                    onChange={() => toggleId(chunk.id)}
                                    onClick={e => e.stopPropagation()}
                                    className="size-3.5 cursor-pointer rounded accent-primary"
                                    aria-label={`Select ${chunk.title}`}
                                />
                                <div className="mt-0.5 size-2 shrink-0 rounded-full bg-current opacity-60" style={{ color: dotColor }} />
                            </div>

                            {/* Content — clicking navigates */}
                            <Link
                                to="/chunks/$chunkId"
                                params={{ chunkId: chunk.id }}
                                className="min-w-0 flex-1"
                            >
                                <div className="flex flex-wrap items-center gap-2">
                                    <span className="truncate text-sm font-medium text-foreground">
                                        {chunk.title}
                                    </span>
                                    <span className={`shrink-0 rounded border px-1.5 py-0.5 text-[10px] font-medium ${typeColor}`}>
                                        {chunk.type}
                                    </span>
                                    {typeof chunk.healthScore === "number" && (
                                        <span className={`shrink-0 text-[10px] font-mono font-bold ${
                                            chunk.healthScore >= 70 ? "text-emerald-500" :
                                            chunk.healthScore >= 40 ? "text-amber-500" :
                                            "text-red-500"
                                        }`}>
                                            {chunk.healthScore}
                                        </span>
                                    )}
                                    {chunk.graphContext && (
                                        <span className="shrink-0 rounded border border-amber-500/30 bg-amber-500/10 px-1.5 py-0.5 text-[10px] font-medium text-amber-400">
                                            {typeof chunk.graphContext.hopDistance === "number"
                                                ? `${chunk.graphContext.hopDistance} hop${chunk.graphContext.hopDistance !== 1 ? "s" : ""} away`
                                                : typeof chunk.graphContext.pathPosition === "number"
                                                ? `path pos ${chunk.graphContext.pathPosition}`
                                                : "in neighborhood"}
                                        </span>
                                    )}
                                </div>

                                {chunk.summary && (
                                    <p className="mt-0.5 line-clamp-1 text-xs text-muted-foreground">
                                        {chunk.summary}
                                    </p>
                                )}

                                <div className="mt-1.5 flex flex-wrap items-center gap-2">
                                    {chunk.tags && chunk.tags.length > 0 && (
                                        <div className="flex flex-wrap gap-1">
                                            {chunk.tags.slice(0, 4).map(tag => (
                                                <span
                                                    key={tag.name}
                                                    className="rounded bg-muted px-1.5 py-0.5 text-[10px] text-muted-foreground"
                                                >
                                                    {tag.name}
                                                </span>
                                            ))}
                                            {chunk.tags.length > 4 && (
                                                <span className="text-[10px] text-muted-foreground">
                                                    +{chunk.tags.length - 4}
                                                </span>
                                            )}
                                        </div>
                                    )}

                                    {typeof chunk.connectionCount === "number" && chunk.connectionCount > 0 && (
                                        <span className="flex items-center gap-0.5 text-[10px] text-muted-foreground">
                                            <Cable className="size-2.5" />
                                            {chunk.connectionCount}
                                        </span>
                                    )}

                                    {chunk.updatedAt && (
                                        <span className="flex items-center gap-0.5 text-[10px] text-muted-foreground">
                                            <Clock className="size-2.5" />
                                            {formatRelativeTime(chunk.updatedAt)}
                                        </span>
                                    )}
                                </div>
                            </Link>
                        </div>
                    );
                })}
            </div>

            {/* Bulk action bar (fixed, bottom-center) */}
            <SearchBulkActionBar selectedIds={selectedIds} onClear={() => setSelectedIds(new Set())} />
        </div>
    );
}
