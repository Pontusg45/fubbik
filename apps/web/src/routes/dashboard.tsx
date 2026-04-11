import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute, Link } from "@tanstack/react-router";
import {
    AlertTriangle,
    ArrowRight,
    Blocks,
    ChevronDown,
    ClipboardList,
    Clock,
    Code,
    Download,
    Eye,
    FileCode,
    FileText,
    Flag,
    Network,
    Plus,
    Star,
    Tags,
    Activity,
    Upload,
    Workflow,
    Zap
} from "lucide-react";
import { useRef, useState } from "react";
import { toast } from "sonner";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";
import { Skeleton } from "@/components/ui/skeleton";
import { SkeletonList } from "@/components/ui/skeleton-list";
import { FeaturedChunkWidget } from "@/features/chunks/featured-chunk-widget";
import { SmartCollections } from "@/features/chunks/smart-collections";
import { MissedChunksWidget } from "@/features/dashboard/missed-chunks-widget";
import { useFavorites } from "@/features/chunks/use-favorites";
import { MilestoneCards } from "@/features/onboarding/milestone-cards";
import { WelcomeWizard } from "@/features/onboarding/welcome-wizard";
import { AttentionNeeded } from "@/features/staleness/attention-needed";
import { useRecentChunks } from "@/features/chunks/use-recent-chunks";
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
    const { recentIds } = useRecentChunks();
    const { codebaseId } = useActiveCodebase();
    const [showWelcome, setShowWelcome] = useState(() => {
        return !localStorage.getItem("fubbik-welcome-shown");
    });

    const dismissWelcome = () => {
        setShowWelcome(false);
        localStorage.setItem("fubbik-welcome-shown", "true");
    };

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
        queryFn: async () => unwrapEden(await api.api.activity.get({ query: { limit: "15" } as any }))
    });

    const plansQuery = useQuery({
        queryKey: ["dashboard-plans", codebaseId],
        queryFn: async () => unwrapEden(await api.api.plans.get({ query: { ...codebaseQuery, status: "active" } as any }))
    });

    const sessionsQuery = useQuery({
        queryKey: ["dashboard-sessions"],
        queryFn: async () => unwrapEden(await api.api.sessions.get({ query: { status: "completed", limit: "3" } as any }))
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

    const recentChunksQuery = useQuery({
        queryKey: ["chunks-recent-viewed", recentIds],
        queryFn: async () => {
            if (recentIds.length === 0) return [];
            const results = await Promise.all(
                recentIds.map(async (id) => {
                    try {
                        return unwrapEden(await api.api.chunks({ id }).get());
                    } catch {
                        return null;
                    }
                })
            );
            return results.filter((c): c is NonNullable<typeof c> => !!c);
        },
        enabled: recentIds.length > 0
    });

    const entryPointsQuery = useQuery({
        queryKey: ["entry-points", codebaseId],
        queryFn: async () => {
            const result = unwrapEden(await api.api.chunks.get({ query: { ...codebaseQuery, limit: "500" } as any }));
            const chunks = ((result as any)?.chunks ?? []) as any[];
            return chunks.filter(c => c.isEntryPoint === true);
        }
    });

    // ─── Mutations ───

    const exportJsonMutation = useMutation({
        mutationFn: async () => unwrapEden(await api.api.chunks.export.get()),
        onSuccess: data => {
            const blob = new Blob([JSON.stringify(data, null, 2)], { type: "application/json" });
            const url = URL.createObjectURL(blob);
            const a = document.createElement("a");
            a.href = url;
            a.download = `fubbik-export-${new Date().toISOString().slice(0, 10)}.json`;
            a.click();
            URL.revokeObjectURL(url);
            toast.success(`Exported ${Array.isArray(data) ? data.length : 0} chunks as JSON`);
        },
        onError: () => toast.error("Failed to export")
    });

    const exportMarkdownMutation = useMutation({
        mutationFn: async () => unwrapEden(await api.api.chunks.export.get()),
        onSuccess: data => {
            const chunks = Array.isArray(data) ? data : [];
            const md = chunks.map((c: any) =>
                `# ${c.title}\n\n**Type:** ${c.type}\n**Tags:** ${(c.tags || []).map((t: any) => t.name || t).join(", ")}\n\n${c.content || ""}`
            ).join("\n\n---\n\n");
            const blob = new Blob([md], { type: "text/markdown" });
            const url = URL.createObjectURL(blob);
            const a = document.createElement("a");
            a.href = url;
            a.download = `fubbik-export-${new Date().toISOString().slice(0, 10)}.md`;
            a.click();
            URL.revokeObjectURL(url);
            toast.success(`Exported ${chunks.length} chunks as Markdown`);
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

    // Show onboarding wizard on first visit or when the knowledge base is empty
    if (!statsQuery.isLoading && (showWelcome || statsQuery.data?.chunks === 0)) {
        return (
            <div className="container mx-auto max-w-6xl px-4 py-8">
                <WelcomeWizard />
                {showWelcome && (
                    <div className="mt-4 text-center">
                        <Button variant="ghost" size="sm" onClick={dismissWelcome}>
                            Skip setup and go to dashboard
                        </Button>
                    </div>
                )}
            </div>
        );
    }

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
                    <DropdownMenu>
                        <DropdownMenuTrigger
                            className="inline-flex items-center justify-center gap-1 rounded-md px-2.5 py-1.5 text-sm font-medium transition-colors hover:bg-accent hover:text-accent-foreground disabled:pointer-events-none disabled:opacity-50"
                        >
                            <Download className="size-3.5" />
                            <ChevronDown className="size-3" />
                        </DropdownMenuTrigger>
                        <DropdownMenuContent align="end">
                            <DropdownMenuItem onClick={() => exportJsonMutation.mutate()}>
                                <FileCode className="size-4" />
                                Export as JSON
                            </DropdownMenuItem>
                            <DropdownMenuItem onClick={() => exportMarkdownMutation.mutate()}>
                                <FileText className="size-4" />
                                Export as Markdown
                            </DropdownMenuItem>
                        </DropdownMenuContent>
                    </DropdownMenu>
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
                <StatCard icon={Blocks} label="Chunks" value={statsQuery.data?.chunks} loading={statsQuery.isLoading} to="/chunks" />
                <StatCard icon={Network} label="Connections" value={statsQuery.data?.connections} loading={statsQuery.isLoading} to="/graph" />
                <StatCard icon={Tags} label="Tags" value={statsQuery.data?.tags} loading={statsQuery.isLoading} to="/tags" />
                <Link to="/requirements">
                    <div className="bg-card hover:bg-muted/50 cursor-pointer rounded-lg border p-4 transition-colors">
                        <div className="flex items-center gap-2">
                            <ClipboardList className="text-muted-foreground size-4" />
                            <span className="text-muted-foreground text-xs font-medium uppercase tracking-wide">Requirements</span>
                        </div>
                        <div className="mt-2 text-2xl font-bold tabular-nums tracking-tight">
                            {requirementsQuery.isLoading ? <Skeleton className="h-8 w-16" /> : (reqStats as any)?.total ?? 0}
                        </div>
                        {reqStats && (
                            <div className="mt-1 flex gap-2 text-[11px]">
                                <span className="text-emerald-500">{(reqStats as any).passing ?? 0} passing</span>
                                <span className="text-red-500">{(reqStats as any).failing ?? 0} failing</span>
                                <span className="text-muted-foreground">{(reqStats as any).untested ?? 0} untested</span>
                            </div>
                        )}
                    </div>
                </Link>
            </div>

            {/* Onboarding milestones */}
            <MilestoneCards stats={statsQuery.data ? {
                chunks: statsQuery.data.chunks,
                connections: statsQuery.data.connections,
            } : null} />

            {/* Stale chunk alerts */}
            <AttentionNeeded />

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

                    {/* Start here — entry points */}
                    {entryPointsQuery.data && entryPointsQuery.data.length > 0 && (
                        <DashboardSection
                            icon={Flag}
                            title="Start here"
                            iconClass="text-emerald-500"
                        >
                            <div className="space-y-1">
                                {entryPointsQuery.data.map((chunk: any) => (
                                    <Link
                                        key={chunk.id}
                                        to="/chunks/$chunkId"
                                        params={{ chunkId: chunk.id }}
                                        className="flex items-center gap-2 rounded-md px-2 py-1.5 hover:bg-muted transition-colors"
                                    >
                                        <Flag className="size-3 shrink-0 text-emerald-500/60" />
                                        <span className="truncate text-sm">{chunk.title}</span>
                                        <Badge variant="secondary" size="sm" className="ml-auto shrink-0 font-mono text-[9px]">
                                            {chunk.type}
                                        </Badge>
                                    </Link>
                                ))}
                            </div>
                        </DashboardSection>
                    )}

                    {/* Recently viewed */}
                    {recentIds.length > 0 && (
                        <DashboardSection icon={Eye} title="Recently Viewed">
                            <div className="grid gap-1 sm:grid-cols-2">
                                {(recentChunksQuery.data ?? []).slice(0, 6).map(item => (
                                    <Link
                                        key={item.chunk.id}
                                        to="/chunks/$chunkId"
                                        params={{ chunkId: item.chunk.id }}
                                        className="hover:bg-muted/50 flex items-center gap-2 rounded-md px-3 py-2 transition-colors"
                                    >
                                        <Eye className="size-3 shrink-0 text-muted-foreground" />
                                        <span className="truncate text-sm">{item.chunk.title}</span>
                                        <Badge variant="secondary" size="sm" className="ml-auto shrink-0 font-mono text-[9px]">
                                            {item.chunk.type}
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
                            <SkeletonList count={5} />
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
                    {/* Featured chunk of the day */}
                    <FeaturedChunkWidget />

                    {/* Smart collections */}
                    <SmartCollections />

                    {/* You might have missed this */}
                    <MissedChunksWidget />

                    {/* Quick Actions */}
                    <div className="mb-0">
                        <h3 className="text-sm font-semibold mb-3 flex items-center gap-2">
                            <Zap className="size-4" />
                            Quick Actions
                        </h3>
                        <div className="grid grid-cols-2 gap-2">
                            <Button variant="outline" size="sm" className="justify-start" render={<Link to="/chunks/new" />}>
                                <Plus className="size-3.5" />
                                New Chunk
                            </Button>
                            <Button variant="outline" size="sm" className="justify-start" render={<Link to="/import" />}>
                                <FileText className="size-3.5" />
                                Import Docs
                            </Button>
                            <Button variant="outline" size="sm" className="justify-start" render={<Link to="/graph" />}>
                                <Network className="size-3.5" />
                                View Graph
                            </Button>
                            <Button variant="outline" size="sm" className="justify-start" render={<Link to="/knowledge-health" />}>
                                <Activity className="size-3.5" />
                                Health Check
                            </Button>
                        </div>
                    </div>

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
                            <div className="space-y-2">
                                <Skeleton className="h-4 w-full" />
                                <Skeleton className="h-4 w-3/4" />
                                <Skeleton className="h-4 w-1/2" />
                            </div>
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

                    {/* Active Plans */}
                    <DashboardSection
                        icon={ClipboardList}
                        title="Active Plans"
                        action={
                            <Link to="/plans" className="text-muted-foreground hover:text-foreground flex items-center gap-1 text-xs transition-colors">
                                View all <ArrowRight className="size-3" />
                            </Link>
                        }
                    >
                        {plansQuery.isLoading ? (
                            <SkeletonList count={3} />
                        ) : (() => {
                            const plans = (plansQuery.data as any)?.plans ?? (Array.isArray(plansQuery.data) ? plansQuery.data : []);
                            if (plans.length === 0) {
                                return <p className="text-muted-foreground py-2 text-center text-sm">No active plans</p>;
                            }
                            return (
                                <div className="space-y-2">
                                    {plans.slice(0, 5).map((plan: any) => {
                                        const steps: any[] = plan.steps ?? [];
                                        const total = steps.length > 0 ? steps.length : (plan.stepCount ?? 0);
                                        const done = steps.length > 0
                                            ? steps.filter((s: any) => s.status === "done").length
                                            : (plan.completedStepCount ?? 0);
                                        const pct = total > 0 ? Math.round((done / total) * 100) : 0;
                                        return (
                                            <Link
                                                key={plan.id}
                                                to="/plans/$planId"
                                                params={{ planId: plan.id }}
                                                className="hover:bg-muted/50 block rounded-md px-2 py-1.5 transition-colors"
                                            >
                                                <p className="truncate text-sm font-medium">{plan.title}</p>
                                                {total > 0 && (
                                                    <div className="mt-1 flex items-center gap-2">
                                                        <div className="bg-muted h-1.5 flex-1 rounded-full overflow-hidden">
                                                            <div
                                                                className="bg-emerald-500 h-full rounded-full transition-all"
                                                                style={{ width: `${pct}%` }}
                                                            />
                                                        </div>
                                                        <span className="text-muted-foreground shrink-0 text-[10px]">{done}/{total}</span>
                                                    </div>
                                                )}
                                            </Link>
                                        );
                                    })}
                                </div>
                            );
                        })()}
                    </DashboardSection>

                    {/* Recent Sessions */}
                    <DashboardSection
                        icon={Workflow}
                        title="Recent Sessions"
                        action={
                            <Link to="/reviews" className="text-muted-foreground hover:text-foreground flex items-center gap-1 text-xs transition-colors">
                                View all <ArrowRight className="size-3" />
                            </Link>
                        }
                    >
                        {sessionsQuery.isLoading ? (
                            <SkeletonList count={3} />
                        ) : (() => {
                            const sessions = (sessionsQuery.data as any)?.sessions ?? (Array.isArray(sessionsQuery.data) ? sessionsQuery.data : []);
                            if (sessions.length === 0) {
                                return <p className="text-muted-foreground py-2 text-center text-sm">No recent sessions</p>;
                            }
                            return (
                                <div className="space-y-1">
                                    {sessions.slice(0, 3).map((session: any) => (
                                        <Link
                                            key={session.id}
                                            to="/reviews/$sessionId"
                                            params={{ sessionId: session.id }}
                                            className="hover:bg-muted/50 flex items-center gap-2 rounded-md px-2 py-1.5 transition-colors"
                                        >
                                            <div className="min-w-0 flex-1">
                                                <p className="truncate text-sm">{session.title}</p>
                                                {session.completedAt && (
                                                    <p className="text-muted-foreground text-[10px]">{timeAgo(session.completedAt)}</p>
                                                )}
                                            </div>
                                            <Badge variant="secondary" size="sm" className="shrink-0 font-mono text-[9px]">
                                                {session.status}
                                            </Badge>
                                        </Link>
                                    ))}
                                </div>
                            );
                        })()}
                    </DashboardSection>

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
                            <ActivityFeed activities={activities} />
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

function StatCard({ icon: Icon, label, value, loading, sub, to }: {
    icon: typeof Blocks;
    label: string;
    value?: number;
    loading: boolean;
    sub?: string;
    to?: string;
}) {
    const content = (
        <div className={`bg-card rounded-lg border p-4${to ? " hover:bg-muted/50 transition-colors cursor-pointer" : ""}`}>
            <div className="flex items-center gap-2">
                <Icon className="text-muted-foreground size-4" />
                <span className="text-muted-foreground text-xs font-medium uppercase tracking-wide">{label}</span>
            </div>
            <div className="mt-2 text-2xl font-bold tabular-nums tracking-tight">
                {loading ? <Skeleton className="h-8 w-16" /> : value ?? 0}
            </div>
            {sub && <div className="text-muted-foreground mt-0.5 text-[11px]">{sub}</div>}
        </div>
    );

    if (to) {
        return <Link to={to as any}>{content}</Link>;
    }

    return content;
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

const ACTIVITY_ENTITY_ICONS: Record<string, typeof Blocks> = {
    chunk: FileText,
    requirement: Code,
    plan: ClipboardList,
    session: Workflow,
};

const ACTIVITY_ACTION_COLORS: Record<string, string> = {
    created: "text-emerald-500",
    updated: "text-blue-500",
    deleted: "text-red-500",
    archived: "text-yellow-500",
};

function getActivityLink(entityType: string, entityId: string): string | null {
    switch (entityType) {
        case "chunk": return `/chunks/${entityId}`;
        case "requirement": return `/requirements/${entityId}`;
        case "plan": return `/plans/${entityId}`;
        case "session": return `/reviews/${entityId}`;
        default: return null;
    }
}

function groupActivitiesByDate(activities: any[]): { label: string; items: any[] }[] {
    const now = new Date();
    const today = new Date(now.getFullYear(), now.getMonth(), now.getDate());
    const yesterday = new Date(today.getTime() - 86400000);

    const groups: { label: string; items: any[] }[] = [];
    const buckets: Record<string, any[]> = { Today: [], Yesterday: [], Earlier: [] };

    for (const a of activities) {
        const d = new Date(a.createdAt);
        if (d >= today) buckets.Today.push(a);
        else if (d >= yesterday) buckets.Yesterday.push(a);
        else buckets.Earlier.push(a);
    }

    for (const label of ["Today", "Yesterday", "Earlier"] as const) {
        if (buckets[label].length > 0) {
            groups.push({ label, items: buckets[label] });
        }
    }
    return groups;
}

function ActivityFeed({ activities }: { activities: any[] }) {
    const groups = groupActivitiesByDate(activities);

    return (
        <div className="space-y-3">
            {groups.map(group => (
                <div key={group.label}>
                    <div className="text-muted-foreground mb-1.5 text-[10px] font-semibold uppercase tracking-wider">{group.label}</div>
                    <div className="space-y-1">
                        {group.items.map((a: any) => {
                            const Icon = ACTIVITY_ENTITY_ICONS[a.entityType] ?? Blocks;
                            const actionColor = ACTIVITY_ACTION_COLORS[a.action] ?? "text-muted-foreground";
                            const link = getActivityLink(a.entityType, a.entityId);
                            const content = (
                                <div className="hover:bg-muted/50 flex items-center gap-2 rounded-md px-2 py-1.5 text-xs transition-colors">
                                    <Icon className={`size-3.5 shrink-0 ${actionColor}`} />
                                    <span className="min-w-0 flex-1 truncate">
                                        <span className={`font-medium capitalize ${actionColor}`}>{a.action}</span>{" "}
                                        <span className="text-foreground">{a.entityTitle ?? a.entityType}</span>
                                    </span>
                                    <span className="text-muted-foreground shrink-0 text-[10px]">{timeAgo(a.createdAt)}</span>
                                </div>
                            );

                            return link ? (
                                <Link key={a.id} to={link as any}>
                                    {content}
                                </Link>
                            ) : (
                                <div key={a.id}>{content}</div>
                            );
                        })}
                    </div>
                </div>
            ))}
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
