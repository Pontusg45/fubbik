import { useQuery } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import { useMemo, useState } from "react";
import { AlphabeticalIndex } from "@/features/browse/alphabetical-index";
import { TagCloud } from "@/features/browse/tag-cloud";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export const Route = createFileRoute("/browse")({
    component: BrowsePage,
});

function BrowsePage() {
    const [view, setView] = useState<"alphabetical" | "tags">("alphabetical");

    const chunksQuery = useQuery({
        queryKey: ["browse-chunks"],
        queryFn: async () => unwrapEden(await api.api.chunks.get({ query: { limit: "1000" } as any })),
    });

    const graphQuery = useQuery({
        queryKey: ["browse-graph-tags"],
        queryFn: async () => unwrapEden(await api.api.graph.get({ query: {} })),
        enabled: view === "tags",
    });

    const chunks = ((chunksQuery.data as any)?.chunks ?? []) as Array<{ id: string; title: string; type: string }>;

    const tagsForCloud = useMemo(() => {
        const chunkTags = ((graphQuery.data as any)?.chunkTags ?? []) as Array<{ tagName: string }>;
        if (chunkTags.length === 0) return [];
        const counts = new Map<string, number>();
        for (const ct of chunkTags) {
            counts.set(ct.tagName, (counts.get(ct.tagName) ?? 0) + 1);
        }
        return Array.from(counts.entries())
            .map(([name, count]) => ({ name, count }))
            .sort((a, b) => b.count - a.count);
    }, [graphQuery.data]);

    return (
        <div className="container mx-auto max-w-6xl px-4 py-8">
            <div className="mb-6">
                <h1 className="text-2xl font-bold tracking-tight">Browse</h1>
                <p className="text-muted-foreground text-sm">Explore chunks by letter or tag</p>
            </div>

            {/* Tab toggle */}
            <div className="mb-6 flex gap-2">
                <button
                    type="button"
                    onClick={() => setView("alphabetical")}
                    className={`text-sm rounded px-3 py-1.5 transition-colors ${view === "alphabetical" ? "bg-muted font-semibold" : "hover:bg-muted/50"}`}
                >
                    A-Z
                </button>
                <button
                    type="button"
                    onClick={() => setView("tags")}
                    className={`text-sm rounded px-3 py-1.5 transition-colors ${view === "tags" ? "bg-muted font-semibold" : "hover:bg-muted/50"}`}
                >
                    Tag cloud
                </button>
            </div>

            {view === "alphabetical" ? (
                <AlphabeticalIndex chunks={chunks} />
            ) : (
                <TagCloud tags={tagsForCloud} />
            )}
        </div>
    );
}
