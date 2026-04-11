import { useQuery } from "@tanstack/react-query";
import { createFileRoute, Link } from "@tanstack/react-router";
import { Blocks, Clock, Network, Tag } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export const Route = createFileRoute("/codebases/$codebaseId")({
    component: CodebaseDashboard,
});

function CodebaseDashboard() {
    const { codebaseId } = Route.useParams();

    const statsQuery = useQuery({
        queryKey: ["codebase-stats", codebaseId],
        queryFn: async () => unwrapEden(await api.api.stats.get({ query: { codebaseId } as any })),
    });

    const chunksQuery = useQuery({
        queryKey: ["codebase-chunks", codebaseId],
        queryFn: async () => unwrapEden(await api.api.chunks.get({ query: { codebaseId, limit: "10", sort: "updated" } as any })),
    });

    const codebaseQuery = useQuery({
        queryKey: ["codebase", codebaseId],
        queryFn: async () => unwrapEden(await api.api.codebases({ id: codebaseId }).get()),
    });

    const codebase = codebaseQuery.data as any;
    const stats = statsQuery.data as any;
    const chunks = ((chunksQuery.data as any)?.chunks ?? []) as Array<{ id: string; title: string; type: string; updatedAt: string }>;

    return (
        <div className="container mx-auto max-w-6xl px-4 py-8">
            <div className="mb-8">
                <h1 className="text-2xl font-bold tracking-tight">{codebase?.name ?? "Codebase"}</h1>
                {codebase?.remoteUrl && (
                    <p className="text-xs text-muted-foreground font-mono mt-1">{codebase.remoteUrl}</p>
                )}
            </div>

            {/* Stats */}
            <div className="mb-8 grid grid-cols-2 gap-3 sm:grid-cols-4">
                <StatCard icon={Blocks} label="Chunks" value={stats?.chunks} />
                <StatCard icon={Network} label="Connections" value={stats?.connections} />
                <StatCard icon={Tag} label="Tags" value={stats?.tags} />
                <StatCard icon={Clock} label="Updated" value={codebase?.updatedAt ? new Date(codebase.updatedAt).toLocaleDateString() : "—"} />
            </div>

            {/* Recent chunks */}
            <div className="rounded-lg border">
                <div className="border-b px-4 py-3">
                    <h2 className="text-sm font-semibold">Recent chunks</h2>
                </div>
                {chunks.length === 0 ? (
                    <div className="p-6 text-center text-sm text-muted-foreground">
                        No chunks in this codebase yet.
                    </div>
                ) : (
                    <div className="divide-y">
                        {chunks.map(chunk => (
                            <Link
                                key={chunk.id}
                                to="/chunks/$chunkId"
                                params={{ chunkId: chunk.id }}
                                className="flex items-center justify-between px-4 py-3 hover:bg-muted/50 transition-colors"
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
        <div className="rounded-lg border bg-card p-4">
            <div className="flex items-center gap-2">
                <Icon className="text-muted-foreground size-4" />
                <span className="text-muted-foreground text-xs font-medium uppercase tracking-wide">{label}</span>
            </div>
            <div className="mt-2 text-2xl font-bold tabular-nums">
                {typeof value === "number" ? value : String(value ?? "—")}
            </div>
        </div>
    );
}
