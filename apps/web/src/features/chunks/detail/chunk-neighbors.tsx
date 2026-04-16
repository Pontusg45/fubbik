import { useQuery } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { Compass } from "lucide-react";

import { Badge } from "@/components/ui/badge";
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

    if (note) {
        return (
            <section className="mt-10 border-t pt-6">
                <header className="mb-3 flex items-center gap-1.5 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                    <Compass className="size-3.5" />
                    Semantic neighbors
                </header>
                <p className="text-sm text-muted-foreground">{note}</p>
            </section>
        );
    }

    if (neighbors.length === 0) return null;

    return (
        <section className="mt-10 border-t pt-6">
            <header className="mb-3 flex items-center gap-1.5 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                <Compass className="size-3.5" />
                Semantic neighbors
            </header>
            <ul className="space-y-1.5">
                {neighbors.map(n => {
                    const pct = similarityBar(n.distance);
                    return (
                        <li key={n.id}>
                            <Link
                                to="/chunks/$chunkId"
                                params={{ chunkId: n.id }}
                                className="hover:bg-muted/60 flex items-center gap-3 rounded px-2 py-1.5 text-sm"
                            >
                                <span className="w-10 shrink-0 font-mono text-xs text-muted-foreground tabular-nums">
                                    {pct}%
                                </span>
                                <span className="truncate font-medium">{n.title}</span>
                                <Badge variant="secondary" size="sm" className="ml-auto shrink-0">
                                    {n.type}
                                </Badge>
                            </Link>
                        </li>
                    );
                })}
            </ul>
        </section>
    );
}
