import { useQuery } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { Compass } from "lucide-react";

import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

interface ChunkNeighborsProps {
    chunkId: string;
}

type Neighbor = {
    id: string;
    title: string;
    type: string;
    summary: string | null;
    distance: number;
};

function similarityBar(distance: number) {
    const similarity = Math.max(0, Math.min(1, 1 - distance / 2));
    return Math.round(similarity * 100);
}

export function ChunkNeighbors({ chunkId }: ChunkNeighborsProps) {
    const neighborsQuery = useQuery({
        queryKey: ["chunk-neighbors", chunkId],
        queryFn: async () => unwrapEden(await api.api.chunks({ id: chunkId }).neighbors.get({ query: { k: "10" } }))
    });

    const data = neighborsQuery.data;
    if (!data) return null;
    const note = data.note;
    const neighbors = (data.neighbors ?? []) as Neighbor[];

    if (note || neighbors.length === 0) return null;

    return (
        <div className="border-t pt-4">
            <div className="text-[9px] font-semibold uppercase tracking-wider text-muted-foreground mb-2 flex items-center gap-1">
                <Compass className="size-3" />
                Neighbors
            </div>
            <ul className="space-y-0.5">
                {neighbors.map(n => {
                    const pct = similarityBar(n.distance);
                    return (
                        <li key={n.id}>
                            <Link
                                to="/chunks/$chunkId"
                                params={{ chunkId: n.id }}
                                className="hover:bg-muted/60 group flex items-center gap-1.5 rounded px-1 py-1 text-[11px]"
                            >
                                <span className="shrink-0 font-mono text-[10px] text-muted-foreground tabular-nums w-7">
                                    {pct}%
                                </span>
                                <span className="min-w-0 truncate">{n.title}</span>
                            </Link>
                        </li>
                    );
                })}
            </ul>
        </div>
    );
}
