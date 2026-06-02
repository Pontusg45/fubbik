import { useQuery } from "@tanstack/react-query";
import { createFileRoute, Link } from "@tanstack/react-router";
import { Blocks, Clock, Network, Tag } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export const Route = createFileRoute("/spaces/$spaceId")({
    component: SpaceDashboard
});

function SpaceDashboard() {
    const { spaceId } = Route.useParams();

    const statsQuery = useQuery({
        queryKey: ["space-stats", spaceId],
        queryFn: async () => unwrapEden(await api.api.stats.get({ query: { spaceId } as any }))
    });

    const chunksQuery = useQuery({
        queryKey: ["space-chunks", spaceId],
        queryFn: async () => unwrapEden(await api.api.chunks.get({ query: { spaceId, limit: "10", sort: "updated" } as any }))
    });

    const spaceQuery = useQuery({
        queryKey: ["space", spaceId],
        queryFn: async () => unwrapEden(await api.api.spaces({ id: spaceId }).get())
    });

    const space = spaceQuery.data as any;
    const stats = statsQuery.data as any;
    const chunks = ((chunksQuery.data as any)?.chunks ?? []) as Array<{ id: string; title: string; type: string; updatedAt: string }>;

    return (
        <div className="container mx-auto max-w-6xl px-4 py-8">
            <div className="mb-8">
                <h1 className="text-2xl font-bold tracking-tight">{space?.name ?? "Space"}</h1>
                {space?.remoteUrl && <p className="text-muted-foreground mt-1 font-mono text-xs">{space.remoteUrl}</p>}
            </div>

            {/* Stats */}
            <div className="mb-8 grid grid-cols-2 gap-3 sm:grid-cols-4">
                <StatCard icon={Blocks} label="Chunks" value={stats?.chunks} />
                <StatCard icon={Network} label="Connections" value={stats?.connections} />
                <StatCard icon={Tag} label="Tags" value={stats?.tags} />
                <StatCard icon={Clock} label="Updated" value={space?.updatedAt ? new Date(space.updatedAt).toLocaleDateString() : "—"} />
            </div>

            {/* Recent chunks */}
            <div className="rounded-lg border">
                <div className="border-b px-4 py-3">
                    <h2 className="text-sm font-semibold">Recent chunks</h2>
                </div>
                {chunks.length === 0 ? (
                    <div className="text-muted-foreground p-6 text-center text-sm">No chunks in this space yet.</div>
                ) : (
                    <div className="divide-y">
                        {chunks.map(chunk => (
                            <Link
                                key={chunk.id}
                                to="/chunks/$chunkId"
                                params={{ chunkId: chunk.id }}
                                className="hover:bg-muted/50 flex items-center justify-between px-4 py-3 transition-colors"
                            >
                                <span className="truncate text-sm">{chunk.title}</span>
                                <Badge variant="secondary" size="sm" className="font-mono text-[9px]">
                                    {chunk.type}
                                </Badge>
                            </Link>
                        ))}
                    </div>
                )}
            </div>
        </div>
    );
}

function StatCard({ icon: Icon, label, value }: { icon: typeof Blocks; label: string; value: unknown }) {
    return (
        <div className="bg-card rounded-lg border p-4">
            <div className="flex items-center gap-2">
                <Icon className="text-muted-foreground size-4" />
                <span className="text-muted-foreground text-xs font-medium tracking-wide uppercase">{label}</span>
            </div>
            <div className="mt-2 text-2xl font-bold tabular-nums">{typeof value === "number" ? value : String(value ?? "—")}</div>
        </div>
    );
}
