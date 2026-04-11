import { useQuery } from "@tanstack/react-query";
import { createFileRoute, Link } from "@tanstack/react-router";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export const Route = createFileRoute("/browse/clusters")({
    component: ClustersPage,
});

function ClustersPage() {
    const { data } = useQuery({
        queryKey: ["chunk-clusters"],
        queryFn: async () => unwrapEden(await api.api.chunks.clusters.get()),
    });

    const clusters = ((data as any) ?? []) as Array<{
        seedId: string;
        seedTitle: string;
        members: Array<{ id: string; title: string; type: string; similarity: number }>;
    }>;

    return (
        <div className="container mx-auto max-w-6xl px-4 py-8">
            <h1 className="mb-6 text-2xl font-bold">Topic Clusters</h1>
            {clusters.length === 0 ? (
                <p className="text-muted-foreground">
                    No clusters found. Chunks need embeddings for clustering to work.
                </p>
            ) : (
                <div className="space-y-8">
                    {clusters.map(c => (
                        <section key={c.seedId} className="rounded-lg border p-4">
                            <Link
                                to="/chunks/$chunkId"
                                params={{ chunkId: c.seedId }}
                                className="text-lg font-semibold hover:text-primary transition-colors"
                            >
                                {c.seedTitle}
                            </Link>
                            <div className="mt-3 grid gap-2 sm:grid-cols-2">
                                {c.members.map(m => (
                                    <Link
                                        key={m.id}
                                        to="/chunks/$chunkId"
                                        params={{ chunkId: m.id }}
                                        className="flex items-center justify-between rounded border px-3 py-2 hover:bg-muted/50 transition-colors"
                                    >
                                        <span className="text-sm truncate">{m.title}</span>
                                        <span className="text-xs text-muted-foreground font-mono">
                                            {Math.round(m.similarity * 100)}%
                                        </span>
                                    </Link>
                                ))}
                            </div>
                        </section>
                    ))}
                </div>
            )}
        </div>
    );
}
