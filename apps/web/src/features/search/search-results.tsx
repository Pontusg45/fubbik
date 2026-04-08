import { Link } from "@tanstack/react-router";
import { Cable, Clock, Network, Search } from "lucide-react";
import { useState } from "react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { SkeletonList } from "@/components/ui/skeleton-list";
import { SearchGraph } from "@/features/search/search-graph";

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

export function SearchResults({ chunks, total, graphMeta, isLoading }: SearchResultsProps) {
    const [showGraph, setShowGraph] = useState(false);

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
            {/* Result count + graph badge + toggle */}
            <div className="mb-3 flex items-center gap-2">
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
                    return (
                        <Link
                            key={chunk.id}
                            to="/chunks/$chunkId"
                            params={{ chunkId: chunk.id }}
                            className="flex items-start gap-3 p-3 transition-colors hover:bg-muted/50"
                        >
                            {/* Type dot */}
                            <div className="mt-1.5 size-2 shrink-0 rounded-full bg-current opacity-60" style={{
                                color: typeColor.includes("blue") ? "#60a5fa"
                                    : typeColor.includes("purple") ? "#c084fc"
                                    : typeColor.includes("green") ? "#4ade80"
                                    : typeColor.includes("orange") ? "#fb923c"
                                    : typeColor.includes("pink") ? "#f472b6"
                                    : "#94a3b8"
                            }} />

                            <div className="min-w-0 flex-1">
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
                            </div>
                        </Link>
                    );
                })}
            </div>
        </div>
    );
}
