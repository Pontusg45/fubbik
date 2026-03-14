import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute, Link } from "@tanstack/react-router";
import {
    AlertTriangle,
    ArrowRight,
    Blocks,
    Clock,
    Download,
    FileText,
    Network,
    Plus,
    Star,
    Tags,
    Upload
} from "lucide-react";
import { useRef } from "react";
import { toast } from "sonner";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { useFavorites } from "@/features/chunks/use-favorites";
import { useActiveCodebase } from "@/features/codebases/use-active-codebase";
import { getUser } from "@/functions/get-user";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export const Route = createFileRoute("/dashboard")({
    component: DashboardPage,
    beforeLoad: async () => {
        let session = null;
        try {
            session = await getUser();
        } catch {}
        return { session };
    }
});

function DashboardPage() {
    const { session } = Route.useRouteContext();
    const queryClient = useQueryClient();
    const fileInputRef = useRef<HTMLInputElement>(null);
    const { favoriteIds, isLoading: favoritesLoading } = useFavorites();
    const { codebaseId } = useActiveCodebase();

    const codebaseQuery = codebaseId
        ? { codebaseId }
        : {};

    // ─── Queries ───

    const statsQuery = useQuery({
        queryKey: ["stats", codebaseId],
        queryFn: async () => unwrapEden(await api.api.stats.get({ query: codebaseQuery as any }))
    });

    const recentQuery = useQuery({
        queryKey: ["dashboard-recent", codebaseId],
        queryFn: async () => unwrapEden(await api.api.chunks.get({ query: { ...codebaseQuery, limit: "8", sort: "updated" } as any }))
    });

    const healthQuery = useQuery({
        queryKey: ["dashboard-health", codebaseId],
        queryFn: async () => unwrapEden(await api.api.health.knowledge.get({ query: codebaseQuery as any }))
    });

    const requirementsQuery = useQuery({
        queryKey: ["dashboard-requirements", codebaseId],
        queryFn: async () => unwrapEden(await api.api.requirements.stats.get({ query: codebaseQuery as any }))
    });

    const activityQuery = useQuery({
        queryKey: ["dashboard-activity"],
        queryFn: async () => unwrapEden(await api.api.activity.get({ query: { limit: "5" } as any }))
    });

    const favoritesChunksQuery = useQuery({
        queryKey: ["chunks-favorites", favoriteIds],
        queryFn: async () => {
            if (favoriteIds.length === 0) return [];
            const result = unwrapEden(await api.api.chunks.get({ query: { limit: "100" } as any }));
            const chunks = result?.chunks ?? [];
            return favoriteIds
                .map(id => chunks.find(c => c.id === id))
                .filter((c): c is NonNullable<typeof c> => !!c);
        },
        enabled: favoriteIds.length > 0
    });

    // ─── Mutations ───

    const exportMutation = useMutation({
        mutationFn: async () => unwrapEden(await api.api.chunks.export.get()),
        onSuccess: data => {
            const blob = new Blob([JSON.stringify(data, null, 2)], { type: "application/json" });
            const url = URL.createObjectURL(blob);
            const a = document.createElement("a");
            a.href = url;
            a.download = `fubbik-export-${new Date().toISOString().slice(0, 10)}.json`;
            a.click();
            URL.revokeObjectURL(url);
            toast.success(`Exported ${Array.isArray(data) ? data.length : 0} chunks`);
        },
        onError: () => toast.error("Failed to export")
    });

    const importMutation = useMutation({
        mutationFn: async (chunks: { title: string; content?: string; type?: string; tags?: string[] }[]) =>
            unwrapEden(await api.api.chunks.import.post({ chunks })),
        onSuccess: data => {
            toast.success(`Imported ${data.imported} chunks`);
            queryClient.invalidateQueries({ queryKey: ["stats"] });
            queryClient.invalidateQueries({ queryKey: ["dashboard-recent"] });
        },
        onError: () => toast.error("Failed to import")
    });

    const handleImportFile = (e: React.ChangeEvent<HTMLInputElement>) => {
        const file = e.target.files?.[0];
        if (!file) return;
        const reader = new FileReader();
        reader.onload = () => {
            try {
                const parsed = JSON.parse(reader.result as string);
                const chunks = Array.isArray(parsed) ? parsed : parsed.chunks;
                if (!Array.isArray(chunks)) {
                    toast.error("Invalid format");
                    return;
                }
                importMutation.mutate(chunks);
            } catch {
                toast.error("Failed to parse JSON");
            }
        };
        reader.readAsText(file);
        e.target.value = "";
    };

    // ─── Derived data ───

    const recentChunks = recentQuery.data?.chunks ?? [];
    const healthData = healthQuery.data;
    const healthTotal = (healthData?.orphans?.count ?? 0) + (healthData?.stale?.count ?? 0) + (healthData?.thin?.count ?? 0);
    const reqStats = requirementsQuery.data;
    const activities = (activityQuery.data as any)?.activities ?? activityQuery.data ?? [];

    return (
        <div className="container mx-auto max-w-6xl px-4 py-8">
            {/* Header */}
            <div className="mb-8 flex items-center justify-between">
                <div>
                    <h1 className="text-2xl font-bold tracking-tight">
                        {session?.user.name ? `Welcome back, ${session.user.name}` : "Dashboard"}
                    </h1>
                    <p className="text-muted-foreground mt-0.5 text-sm">Your knowledge base at a glance.</p>
                </div>
                <div className="flex items-center gap-2">
                    <input ref={fileInputRef} type="file" accept=".json" className="hidden" onChange={handleImportFile} />
                    <Button variant="ghost" size="sm" onClick={() => exportMutation.mutate()} disabled={exportMutation.isPending}>
                        <Download className="size-3.5" />
                    </Button>
                    <Button variant="ghost" size="sm" onClick={() => fileInputRef.current?.click()} disabled={importMutation.isPending}>
                        <Upload className="size-3.5" />
                    </Button>
                    <Button size="sm" render={<Link to="/chunks/new" />}>
                        <Plus className="size-3.5" />
                        New Chunk
                    </Button>
                </div>
            </div>

            {/* Stats row */}
            <div className="mb-8 grid grid-cols-2 gap-3 sm:grid-cols-4">
                <StatCard icon={Blocks} label="Chunks" value={statsQuery.data?.chunks} loading={statsQuery.isLoading} />
                <StatCard icon={Network} label="Connections" value={statsQuery.data?.connections} loading={statsQuery.isLoading} />
                <StatCard icon={Tags} label="Tags" value={statsQuery.data?.tags} loading={statsQuery.isLoading} />
                <StatCard
                    icon={FileText}
                    label="Requirements"
                    value={reqStats ? (reqStats as any).total : undefined}
                    loading={requirementsQuery.isLoading}
                    sub={reqStats ? `${(reqStats as any).passing ?? 0} passing` : undefined}
                />
            </div>

            {/* Main grid */}
            <div className="grid gap-6 lg:grid-cols-3">
                {/* Left column — 2/3 */}
                <div className="space-y-6 lg:col-span-2">
                    {/* Favorites */}
                    {!favoritesLoading && favoriteIds.length > 0 && (
                        <DashboardSection icon={Star} title="Favorites" iconClass="fill-yellow-500 text-yellow-500">
                            <div className="grid gap-1 sm:grid-cols-2">
                                {(favoritesChunksQuery.data ?? []).slice(0, 6).map(chunk => (
                                    <Link
                                        key={chunk.id}
                                        to="/chunks/$chunkId"
                                        params={{ chunkId: chunk.id }}
                                        className="hover:bg-muted/50 flex items-center gap-2 rounded-md px-3 py-2 transition-colors"
                                    >
                                        <Star className="size-3 shrink-0 fill-yellow-500/40 text-yellow-500/40" />
                                        <span className="truncate text-sm">{chunk.title}</span>
                                        <Badge variant="secondary" size="sm" className="ml-auto shrink-0 font-mono text-[9px]">
                                            {chunk.type}
                                        </Badge>
                                    </Link>
                                ))}
                            </div>
                        </DashboardSection>
                    )}

                    {/* Recent chunks */}
                    <DashboardSection
                        icon={Clock}
                        title="Recent Chunks"
                        action={
                            <Link to="/chunks" search={{}} className="text-muted-foreground hover:text-foreground flex items-center gap-1 text-xs transition-colors">
                                View all <ArrowRight className="size-3" />
                            </Link>
                        }
                    >
                        {recentQuery.isLoading ? (
                            <p className="text-muted-foreground py-4 text-center text-sm">Loading...</p>
                        ) : recentChunks.length === 0 ? (
                            <div className="flex flex-col items-center gap-3 py-12">
                                <Blocks className="text-muted-foreground/20 size-10" />
                                <div className="text-center">
                                    <p className="text-muted-foreground font-medium">No chunks yet</p>
                                    <p className="text-muted-foreground/70 mt-1 text-sm">Start building your knowledge base by creating your first chunk.</p>
                                </div>
                                <Button size="sm" render={<Link to="/chunks/new" />}>
                                    <Plus className="size-3.5" />
                                    Create your first chunk
                                </Button>
                            </div>
                        ) : (
                            <div className="divide-border divide-y">
                                {recentChunks.map(chunk => (
                                    <Link
                                        key={chunk.id}
                                        to="/chunks/$chunkId"
                                        params={{ chunkId: chunk.id }}
                                        className="hover:bg-muted/30 flex items-center gap-3 px-1 py-2.5 transition-colors"
                                    >
                                        <div className="min-w-0 flex-1">
                                            <p className="truncate text-sm font-medium">{chunk.title}</p>
                                            <div className="mt-0.5 flex items-center gap-2">
                                                <Badge variant="secondary" size="sm" className="font-mono text-[9px]">
                                                    {chunk.type}
                                                </Badge>
                                                <span className="text-muted-foreground text-[11px]">
                                                    {timeAgo(chunk.updatedAt)}
                                                </span>
                                            </div>
                                        </div>
                                    </Link>
                                ))}
                            </div>
                        )}
                    </DashboardSection>
                </div>

                {/* Right column — 1/3 */}
                <div className="space-y-6">
                    {/* Health summary */}
                    <DashboardSection
                        icon={AlertTriangle}
                        title="Knowledge Health"
                        iconClass={healthTotal > 0 ? "text-amber-500" : "text-emerald-500"}
                        action={
                            <Link to="/knowledge-health" className="text-muted-foreground hover:text-foreground flex items-center gap-1 text-xs transition-colors">
                                Details <ArrowRight className="size-3" />
                            </Link>
                        }
                    >
                        {healthQuery.isLoading ? (
                            <p className="text-muted-foreground py-2 text-center text-sm">Loading...</p>
                        ) : healthTotal === 0 ? (
                            <p className="text-muted-foreground py-2 text-center text-sm">All chunks healthy</p>
                        ) : (
                            <div className="space-y-2">
                                {(healthData?.orphans?.count ?? 0) > 0 && (
                                    <HealthRow label="Orphan chunks" count={healthData!.orphans.count} color="text-red-400" />
                                )}
                                {(healthData?.stale?.count ?? 0) > 0 && (
                                    <HealthRow label="Stale chunks" count={healthData!.stale.count} color="text-amber-400" />
                                )}
                                {(healthData?.thin?.count ?? 0) > 0 && (
                                    <HealthRow label="Thin chunks" count={healthData!.thin.count} color="text-blue-400" />
                                )}
                            </div>
                        )}
                    </DashboardSection>

                    {/* Requirements summary */}
                    {reqStats && (
                        <DashboardSection
                            icon={FileText}
                            title="Requirements"
                            action={
                                <Link to="/requirements" className="text-muted-foreground hover:text-foreground flex items-center gap-1 text-xs transition-colors">
                                    View all <ArrowRight className="size-3" />
                                </Link>
                            }
                        >
                            <div className="grid grid-cols-3 gap-2 text-center">
                                <div>
                                    <div className="text-lg font-bold text-emerald-500">{(reqStats as any).passing ?? 0}</div>
                                    <div className="text-muted-foreground text-[10px] uppercase">Passing</div>
                                </div>
                                <div>
                                    <div className="text-lg font-bold text-red-500">{(reqStats as any).failing ?? 0}</div>
                                    <div className="text-muted-foreground text-[10px] uppercase">Failing</div>
                                </div>
                                <div>
                                    <div className="text-muted-foreground text-lg font-bold">{(reqStats as any).untested ?? 0}</div>
                                    <div className="text-muted-foreground text-[10px] uppercase">Untested</div>
                                </div>
                            </div>
                        </DashboardSection>
                    )}

                    {/* Activity */}
                    <DashboardSection
                        icon={Clock}
                        title="Recent Activity"
                        action={
                            <Link to="/activity" className="text-muted-foreground hover:text-foreground flex items-center gap-1 text-xs transition-colors">
                                View all <ArrowRight className="size-3" />
                            </Link>
                        }
                    >
                        {Array.isArray(activities) && activities.length > 0 ? (
                            <div className="space-y-2">
                                {activities.slice(0, 5).map((a: any) => (
                                    <div key={a.id} className="flex items-center gap-2 text-xs">
                                        <span className="text-muted-foreground shrink-0">{timeAgo(a.createdAt)}</span>
                                        <span className="truncate">
                                            <span className="font-medium capitalize">{a.action}</span>{" "}
                                            <span className="text-muted-foreground">{a.entityTitle ?? a.entityType}</span>
                                        </span>
                                    </div>
                                ))}
                            </div>
                        ) : (
                            <p className="text-muted-foreground py-2 text-center text-sm">No activity yet</p>
                        )}
                    </DashboardSection>
                </div>
            </div>
        </div>
    );
}

/* ─── Components ─── */

function StatCard({ icon: Icon, label, value, loading, sub }: {
    icon: typeof Blocks;
    label: string;
    value?: number;
    loading: boolean;
    sub?: string;
}) {
    return (
        <div className="bg-card rounded-lg border p-4">
            <div className="flex items-center gap-2">
                <Icon className="text-muted-foreground size-4" />
                <span className="text-muted-foreground text-xs font-medium uppercase tracking-wide">{label}</span>
            </div>
            <div className="mt-2 text-2xl font-bold tabular-nums tracking-tight">
                {loading ? "—" : value ?? 0}
            </div>
            {sub && <div className="text-muted-foreground mt-0.5 text-[11px]">{sub}</div>}
        </div>
    );
}

function DashboardSection({ icon: Icon, title, iconClass, action, children }: {
    icon: typeof Blocks;
    title: string;
    iconClass?: string;
    action?: React.ReactNode;
    children: React.ReactNode;
}) {
    return (
        <div className="rounded-lg border">
            <div className="flex items-center justify-between border-b px-4 py-3">
                <div className="flex items-center gap-2">
                    <Icon className={`size-4 ${iconClass ?? "text-muted-foreground"}`} />
                    <h2 className="text-sm font-semibold">{title}</h2>
                </div>
                {action}
            </div>
            <div className="p-4">{children}</div>
        </div>
    );
}

function HealthRow({ label, count, color }: { label: string; count: number; color: string }) {
    return (
        <div className="flex items-center justify-between text-sm">
            <span>{label}</span>
            <span className={`font-mono font-bold ${color}`}>{count}</span>
        </div>
    );
}

function timeAgo(dateStr: string | Date): string {
    const diff = Date.now() - new Date(dateStr).getTime();
    const mins = Math.floor(diff / 60000);
    if (mins < 1) return "just now";
    if (mins < 60) return `${mins}m ago`;
    const hours = Math.floor(mins / 60);
    if (hours < 24) return `${hours}h ago`;
    const days = Math.floor(hours / 24);
    if (days < 7) return `${days}d ago`;
    return new Date(dateStr).toLocaleDateString();
}
