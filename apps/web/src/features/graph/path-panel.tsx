import { ArrowRight, Route, X } from "lucide-react";
import { useMemo, useState } from "react";

import { Badge } from "@/components/ui/badge";
import { relationColor } from "@/features/chunks/relation-colors";

interface PathPanelProps {
    chunks: Array<{ id: string; title: string }>;
    pathStartId: string | null;
    pathEndId: string | null;
    pathResult: {
        pathNodeIds: Set<string>;
        pathEdgeIds: Set<string>;
        length: number;
        path?: string[];
    } | null;
    edges: Array<{ id: string; source: string; target: string; data?: { relation?: string } }>;
    onSetStart: (id: string | null) => void;
    onSetEnd: (id: string | null) => void;
    onClear: () => void;
}

function ChunkSelect({
    label,
    value,
    chunks,
    onChange
}: {
    label: string;
    value: string | null;
    chunks: Array<{ id: string; title: string }>;
    onChange: (id: string | null) => void;
}) {
    const [query, setQuery] = useState("");
    const [open, setOpen] = useState(false);

    const filtered = useMemo(() => {
        if (!query.trim()) return chunks.slice(0, 50);
        const q = query.toLowerCase();
        return chunks.filter(c => c.title.toLowerCase().includes(q)).slice(0, 50);
    }, [chunks, query]);

    const selectedTitle = value ? chunks.find(c => c.id === value)?.title ?? value.slice(0, 8) : null;

    return (
        <div className="relative">
            <label className="text-muted-foreground mb-1 block text-[10px] font-medium tracking-wider uppercase">{label}</label>
            {value ? (
                <div className="bg-background flex items-center gap-1.5 rounded-md border px-2.5 py-1.5 text-xs">
                    <span className="flex-1 truncate font-medium">{selectedTitle}</span>
                    <button
                        onClick={() => {
                            onChange(null);
                            setQuery("");
                        }}
                        className="text-muted-foreground hover:text-foreground shrink-0"
                    >
                        <X className="size-3" />
                    </button>
                </div>
            ) : (
                <div>
                    <input
                        type="text"
                        value={query}
                        onChange={e => {
                            setQuery(e.target.value);
                            setOpen(true);
                        }}
                        onFocus={() => setOpen(true)}
                        onBlur={() => {
                            // Delay to allow click on item
                            setTimeout(() => setOpen(false), 150);
                        }}
                        placeholder="Search chunks..."
                        className="bg-background w-full rounded-md border px-2.5 py-1.5 text-xs focus:ring-2 focus:ring-ring focus:outline-none"
                    />
                    {open && filtered.length > 0 && (
                        <div className="bg-popover absolute z-50 mt-1 max-h-48 w-full overflow-y-auto rounded-md border shadow-lg">
                            {filtered.map(c => (
                                <button
                                    key={c.id}
                                    className="hover:bg-accent w-full truncate px-2.5 py-1.5 text-left text-xs"
                                    onMouseDown={e => {
                                        e.preventDefault();
                                        onChange(c.id);
                                        setQuery("");
                                        setOpen(false);
                                    }}
                                >
                                    {c.title}
                                </button>
                            ))}
                        </div>
                    )}
                </div>
            )}
        </div>
    );
}

export function PathPanel({ chunks, pathStartId, pathEndId, pathResult, edges, onSetStart, onSetEnd, onClear }: PathPanelProps) {
    const chunkTitleMap = useMemo(() => {
        const m = new Map<string, string>();
        for (const c of chunks) m.set(c.id, c.title);
        return m;
    }, [chunks]);

    // Build relation chain when path exists
    const relationChain = useMemo(() => {
        if (!pathResult?.path || pathResult.path.length < 2) return null;
        const chain: Array<{ fromId: string; toId: string; fromTitle: string; toTitle: string; relations: string[]; direction: "forward" | "backward" }> = [];

        for (let i = 0; i < pathResult.path.length - 1; i++) {
            const a = pathResult.path[i]!;
            const b = pathResult.path[i + 1]!;
            const forwardRels: string[] = [];
            const backwardRels: string[] = [];

            for (const edge of edges) {
                if (edge.source === a && edge.target === b) {
                    forwardRels.push(edge.data?.relation ?? "related");
                } else if (edge.source === b && edge.target === a) {
                    backwardRels.push(edge.data?.relation ?? "related");
                }
            }

            const relations = forwardRels.length > 0 ? forwardRels : backwardRels;
            const direction = forwardRels.length > 0 ? "forward" as const : "backward" as const;

            chain.push({
                fromId: a,
                toId: b,
                fromTitle: chunkTitleMap.get(a) ?? a.slice(0, 8),
                toTitle: chunkTitleMap.get(b) ?? b.slice(0, 8),
                relations: [...new Set(relations)],
                direction
            });
        }
        return chain;
    }, [pathResult, edges, chunkTitleMap]);

    return (
        <div className="flex flex-col gap-3">
            <div className="flex items-center justify-between">
                <div className="flex items-center gap-1.5">
                    <Route className="text-muted-foreground size-3.5" />
                    <span className="text-xs font-semibold">Find Path</span>
                </div>
                {(pathStartId || pathEndId) && (
                    <button onClick={onClear} className="text-muted-foreground hover:text-foreground text-[10px]">
                        Clear
                    </button>
                )}
            </div>

            <ChunkSelect label="From" value={pathStartId} chunks={chunks} onChange={onSetStart} />
            <ChunkSelect label="To" value={pathEndId} chunks={chunks} onChange={onSetEnd} />

            {pathStartId && pathEndId && pathResult && relationChain && (
                <div className="space-y-2">
                    <div className="flex items-center gap-2">
                        <Badge variant="secondary" className="text-[10px]">
                            {pathResult.length} {pathResult.length === 1 ? "hop" : "hops"}
                        </Badge>
                    </div>
                    <div className="space-y-1">
                        {relationChain.map((hop, i) => (
                            <div key={i} className="flex items-center gap-1 text-[11px]">
                                <span className="max-w-[80px] truncate font-medium" title={hop.fromTitle}>
                                    {hop.fromTitle}
                                </span>
                                <span className="text-muted-foreground flex shrink-0 items-center gap-0.5">
                                    {hop.direction === "forward" ? (
                                        <>
                                            <ArrowRight className="size-2.5" />
                                            <span
                                                className="font-mono text-[10px]"
                                                style={{ color: relationColor(hop.relations[0] ?? "related") }}
                                            >
                                                {hop.relations.join("/")}
                                            </span>
                                            <ArrowRight className="size-2.5" />
                                        </>
                                    ) : (
                                        <>
                                            <span className="rotate-180">
                                                <ArrowRight className="size-2.5" />
                                            </span>
                                            <span
                                                className="font-mono text-[10px]"
                                                style={{ color: relationColor(hop.relations[0] ?? "related") }}
                                            >
                                                {hop.relations.join("/")}
                                            </span>
                                            <span className="rotate-180">
                                                <ArrowRight className="size-2.5" />
                                            </span>
                                        </>
                                    )}
                                </span>
                                <span className="max-w-[80px] truncate font-medium" title={hop.toTitle}>
                                    {hop.toTitle}
                                </span>
                            </div>
                        ))}
                    </div>
                </div>
            )}

            {pathStartId && pathEndId && !pathResult && (
                <p className="text-xs text-red-500">No path found between these chunks.</p>
            )}
        </div>
    );
}
