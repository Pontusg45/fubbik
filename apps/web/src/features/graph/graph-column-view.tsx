import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";

import { relationColor } from "@/features/chunks/relation-colors";
import type { IslandFormationResult } from "@/features/graph/island-formation";
import type { GraphData } from "@/features/graph/use-graph-data";

interface GraphColumnViewProps {
    islandData: IslandFormationResult;
    data: GraphData | undefined;
    chunkHealthScores: Map<string, number>;
    chunkTags: Map<string, Array<{ name: string; color: string }>>;
    islandColors: Map<string, string>;
    selectedChunkId: string | null;
    onSelectChunk: (id: string | null) => void;
}

function healthColor(score: number): string {
    if (score >= 80) return "#22c55e";
    if (score >= 60) return "#4ade80";
    if (score >= 40) return "#f59e0b";
    return "#ef4444";
}

interface ConnectionLine {
    sourceId: string;
    targetId: string;
    relation: string;
}

export function GraphColumnView({
    islandData,
    data,
    chunkHealthScores,
    chunkTags,
    islandColors,
    selectedChunkId,
    onSelectChunk,
}: GraphColumnViewProps) {
    const containerRef = useRef<HTMLDivElement>(null);
    const [chunkRects, setChunkRects] = useState<Map<string, DOMRect>>(new Map());
    const [containerRect, setContainerRect] = useState<DOMRect | null>(null);

    const chunkMap = useMemo(() => {
        const m = new Map<string, { id: string; title: string; type: string; summary?: string | null }>();
        for (const c of data?.chunks ?? []) {
            m.set(c.id, c);
        }
        return m;
    }, [data?.chunks]);

    const connections = useMemo<ConnectionLine[]>(() => {
        if (!data?.connections) return [];
        return data.connections.map(c => ({
            sourceId: c.sourceId,
            targetId: c.targetId,
            relation: c.relation,
        }));
    }, [data?.connections]);

    const selectedNeighborIds = useMemo(() => {
        if (!selectedChunkId) return new Set<string>();
        const ids = new Set<string>();
        for (const c of connections) {
            if (c.sourceId === selectedChunkId) ids.add(c.targetId);
            if (c.targetId === selectedChunkId) ids.add(c.sourceId);
        }
        return ids;
    }, [selectedChunkId, connections]);

    const visibleIslands = useMemo(
        () => islandData.islands.filter(i => i.chunkIds.length > 0),
        [islandData.islands],
    );

    const measureRects = useCallback(() => {
        if (!containerRef.current) return;
        const cr = containerRef.current.getBoundingClientRect();
        setContainerRect(cr);
        const rects = new Map<string, DOMRect>();
        containerRef.current.querySelectorAll<HTMLElement>("[data-chunk-id]").forEach(el => {
            const id = el.dataset.chunkId!;
            rects.set(id, el.getBoundingClientRect());
        });
        setChunkRects(rects);
    }, []);

    useLayoutEffect(measureRects, [measureRects, visibleIslands]);
    useEffect(() => {
        window.addEventListener("resize", measureRects);
        return () => window.removeEventListener("resize", measureRects);
    }, [measureRects]);
    useEffect(() => {
        const timer = setTimeout(measureRects, 100);
        return () => clearTimeout(timer);
    }, [measureRects, selectedChunkId]);

    const visibleConnections = useMemo(() => {
        if (!selectedChunkId) return [];
        return connections.filter(
            c => c.sourceId === selectedChunkId || c.targetId === selectedChunkId,
        );
    }, [selectedChunkId, connections]);

    const svgLines = useMemo(() => {
        if (!containerRect || chunkRects.size === 0) return [];
        return visibleConnections
            .map(conn => {
                const sr = chunkRects.get(conn.sourceId);
                const tr = chunkRects.get(conn.targetId);
                if (!sr || !tr) return null;
                const sx = sr.left + sr.width - containerRect.left;
                const sy = sr.top + sr.height / 2 - containerRect.top;
                const tx = tr.left - containerRect.left;
                const ty = tr.top + tr.height / 2 - containerRect.top;

                const isReversed = sx > tx;
                const x1 = isReversed ? sr.left - containerRect.left : sx;
                const y1 = sy;
                const x2 = isReversed ? tr.left + tr.width - containerRect.left : tx;
                const y2 = ty;

                const midX = (x1 + x2) / 2;
                return {
                    ...conn,
                    path: `M ${x1} ${y1} C ${midX} ${y1}, ${midX} ${y2}, ${x2} ${y2}`,
                    color: relationColor(conn.relation),
                };
            })
            .filter(Boolean) as Array<ConnectionLine & { path: string; color: string }>;
    }, [visibleConnections, chunkRects, containerRect]);

    return (
        <div ref={containerRef} className="relative flex h-full gap-3 overflow-x-auto p-4">
            {/* SVG overlay for connection lines */}
            <svg className="pointer-events-none absolute inset-0 z-10 h-full w-full">
                {svgLines.map((line, i) => (
                    <path
                        key={i}
                        d={line.path}
                        fill="none"
                        stroke={line.color}
                        strokeWidth={2}
                        strokeOpacity={0.6}
                    />
                ))}
            </svg>

            {visibleIslands.map(island => (
                <div
                    key={island.id}
                    className="flex w-56 shrink-0 flex-col rounded-xl border p-3 sm:w-64"
                    style={{
                        borderColor: `${islandColors.get(island.id) ?? "#334155"}40`,
                        background: `${islandColors.get(island.id) ?? "#334155"}08`,
                    }}
                >
                    {/* Island header */}
                    <div className="mb-3 flex items-center justify-between">
                        <span
                            className="text-xs font-semibold"
                            style={{ color: islandColors.get(island.id) ?? "#94a3b8" }}
                        >
                            {island.name}
                        </span>
                        <span className="text-muted-foreground text-[10px]">{island.chunkIds.length}</span>
                    </div>

                    {/* Chunk list */}
                    <div className="flex flex-col gap-1.5 overflow-y-auto">
                        {island.chunkIds.map(chunkId => {
                            const chunk = chunkMap.get(chunkId);
                            if (!chunk) return null;
                            const isSelected = chunkId === selectedChunkId;
                            const isNeighbor = selectedNeighborIds.has(chunkId);
                            const dimmed = selectedChunkId && !isSelected && !isNeighbor;
                            const health = chunkHealthScores.get(chunkId) ?? 50;
                            const tags = chunkTags.get(chunkId) ?? [];

                            return (
                                <button
                                    key={chunkId}
                                    data-chunk-id={chunkId}
                                    onClick={() => onSelectChunk(isSelected ? null : chunkId)}
                                    className="rounded-lg border p-2 text-left transition-all"
                                    style={{
                                        borderColor: isSelected
                                            ? "#3b82f6"
                                            : isNeighbor
                                              ? `${relationColor("related_to")}60`
                                              : "var(--border)",
                                        opacity: dimmed ? 0.3 : 1,
                                        boxShadow: isSelected ? "0 0 12px rgba(59,130,246,0.2)" : "none",
                                    }}
                                >
                                    <div className="flex items-center gap-1.5">
                                        <div
                                            className="h-1.5 w-1.5 shrink-0 rounded-full"
                                            style={{ background: healthColor(health) }}
                                        />
                                        <span className="text-foreground truncate text-[11px] font-medium">
                                            {chunk.title}
                                        </span>
                                    </div>
                                    {isSelected && tags.length > 0 && (
                                        <div className="mt-1 flex flex-wrap gap-1">
                                            {tags.slice(0, 4).map(tag => (
                                                <span
                                                    key={tag.name}
                                                    className="rounded px-1 py-px text-[8px]"
                                                    style={{
                                                        background: `${tag.color}20`,
                                                        color: tag.color,
                                                    }}
                                                >
                                                    {tag.name}
                                                </span>
                                            ))}
                                        </div>
                                    )}
                                </button>
                            );
                        })}
                    </div>
                </div>
            ))}
        </div>
    );
}
