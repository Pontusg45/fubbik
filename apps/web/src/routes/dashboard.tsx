import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute, Link } from "@tanstack/react-router";
import { Blocks, Clock, Download, Network, Plus, Star, Tags, Upload } from "lucide-react";
import { useRef } from "react";
import { toast } from "sonner";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardDescription, CardHeader, CardPanel, CardTitle } from "@/components/ui/card";
import { Separator } from "@/components/ui/separator";
import { useFavorites } from "@/features/chunks/use-favorites";
import { getUser } from "@/functions/get-user";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export const Route = createFileRoute("/dashboard")({
    component: RouteComponent,
    beforeLoad: async () => {
        let session = null;
        try {
            session = await getUser();
        } catch {
            // allow unauthenticated access
        }
        return { session };
    }
});

function RouteComponent() {
    const { session } = Route.useRouteContext();
    const queryClient = useQueryClient();
    const fileInputRef = useRef<HTMLInputElement>(null);
    const { favoriteIds } = useFavorites();

    const exportMutation = useMutation({
        mutationFn: async () => {
            return unwrapEden(await api.api.chunks.export.get());
        },
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
        onError: () => toast.error("Failed to export chunks")
    });

    const importMutation = useMutation({
        mutationFn: async (chunks: { title: string; content?: string; type?: string; tags?: string[] }[]) => {
            return unwrapEden(await api.api.chunks.import.post({ chunks }));
        },
        onSuccess: data => {
            toast.success(`Imported ${data.imported} chunks`);
            queryClient.invalidateQueries({ queryKey: ["chunks"] });
            queryClient.invalidateQueries({ queryKey: ["stats"] });
        },
        onError: () => toast.error("Failed to import chunks")
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
                    toast.error("Invalid file format: expected an array of chunks");
                    return;
                }
                importMutation.mutate(chunks);
            } catch {
                toast.error("Failed to parse JSON file");
            }
        };
        reader.readAsText(file);
        e.target.value = "";
    };

    const healthCheck = useQuery({
        queryKey: ["health"],
        queryFn: async () => {
            try {
                return unwrapEden(await api.api.health.get());
            } catch {
                return null;
            }
        }
    });

    const statsQuery = useQuery({
        queryKey: ["stats"],
        queryFn: async () => {
            try {
                return unwrapEden(await api.api.stats.get());
            } catch {
                return null;
            }
        }
    });

    const chunksQuery = useQuery({
        queryKey: ["chunks"],
        queryFn: async () => {
            try {
                return unwrapEden(await api.api.chunks.get({ query: { limit: "5" } }));
            } catch {
                return null;
            }
        }
    });

    const favoritesQuery = useQuery({
        queryKey: ["chunks-favorites", favoriteIds],
        queryFn: async () => {
            if (favoriteIds.length === 0) return [];
            try {
                const result = unwrapEden(await api.api.chunks.get({ query: { limit: "100" } }));
                return (result?.chunks ?? []).filter(c => favoriteIds.includes(c.id));
            } catch {
                return [];
            }
        },
        enabled: favoriteIds.length > 0
    });

    const stats = [
        { label: "Chunks", value: statsQuery.data?.chunks ?? 0, icon: Blocks, to: "/dashboard" as const },
        { label: "Connections", value: statsQuery.data?.connections ?? 0, icon: Network, to: "/dashboard" as const },
        { label: "Tags", value: statsQuery.data?.tags ?? 0, icon: Tags, to: "/tags" as const }
    ];

    const recentChunks = chunksQuery.data?.chunks ?? [];

    return (
        <div className="container mx-auto max-w-5xl px-4 py-8">
            <div className="mb-8 flex items-center justify-between">
                <div>
                    <h1 className="text-2xl font-bold tracking-tight">Welcome back{session?.user.name ? `, ${session.user.name}` : ""}</h1>
                    <p className="text-muted-foreground text-sm">Here's an overview of your knowledge base.</p>
                </div>
                <div className="flex items-center gap-2">
                    <input ref={fileInputRef} type="file" accept=".json" className="hidden" onChange={handleImportFile} />
                    <Button variant="outline" onClick={() => exportMutation.mutate()} disabled={exportMutation.isPending}>
                        <Download className="size-4" />
                        Export
                    </Button>
                    <Button variant="outline" onClick={() => fileInputRef.current?.click()} disabled={importMutation.isPending}>
                        <Upload className="size-4" />
                        Import
                    </Button>
                    <Button variant="outline" render={<Link to="/graph" />}>
                        <Network className="size-4" />
                        Graph
                    </Button>
                    <Button render={<Link to="/chunks/new" />}>
                        <Plus className="size-4" />
                        New Chunk
                    </Button>
                </div>
            </div>

            <div className="mb-8 grid grid-cols-2 gap-4 sm:grid-cols-3">
                {stats.map(stat => (
                    <Link key={stat.label} to={stat.to}>
                        <Card>
                            <CardPanel className="hover:bg-muted/50 flex items-center gap-3 p-4 transition-colors">
                                <div className="bg-muted rounded-md p-2">
                                    <stat.icon className="text-muted-foreground size-4" />
                                </div>
                                <div>
                                    <p className="text-2xl font-bold">{statsQuery.isLoading ? "—" : stat.value}</p>
                                    <p className="text-muted-foreground text-xs">{stat.label}</p>
                                </div>
                            </CardPanel>
                        </Card>
                    </Link>
                ))}
            </div>

            {favoriteIds.length > 0 && (
                <div className="mb-6">
                    <div className="mb-3 flex items-center gap-2">
                        <Star className="size-4 fill-yellow-500 text-yellow-500" />
                        <h2 className="font-semibold">Favorites</h2>
                    </div>
                    <Card>
                        {favoritesQuery.isLoading ? (
                            <CardPanel className="p-4 text-center">
                                <p className="text-muted-foreground text-sm">Loading...</p>
                            </CardPanel>
                        ) : (
                            <div className="grid gap-px sm:grid-cols-2">
                                {(favoritesQuery.data ?? []).map(chunk => (
                                    <Link key={chunk.id} to="/chunks/$chunkId" params={{ chunkId: chunk.id }}>
                                        <CardPanel className="hover:bg-muted/50 p-3 transition-colors">
                                            <p className="truncate text-sm font-medium">{chunk.title}</p>
                                            <Badge variant="secondary" size="sm" className="mt-1 font-mono text-[10px]">
                                                {chunk.type}
                                            </Badge>
                                        </CardPanel>
                                    </Link>
                                ))}
                            </div>
                        )}
                    </Card>
                </div>
            )}

            <div className="grid gap-6 lg:grid-cols-3">
                <div className="lg:col-span-2">
                    <div className="mb-3 flex items-center justify-between">
                        <h2 className="font-semibold">Recent Chunks</h2>
                        <Button variant="link" size="sm" render={<Link to="/chunks" search={{ page: 1, type: undefined, q: undefined, sort: undefined, tags: undefined, size: undefined, after: undefined, enrichment: undefined, minConnections: undefined }} />}>
                            View All
                        </Button>
                    </div>
                    <Card>
                        {chunksQuery.isLoading ? (
                            <CardPanel className="p-8 text-center">
                                <p className="text-muted-foreground text-sm">Loading...</p>
                            </CardPanel>
                        ) : recentChunks.length === 0 ? (
                            <CardPanel className="p-8 text-center">
                                <p className="text-muted-foreground text-sm">No chunks yet. Create your first one!</p>
                            </CardPanel>
                        ) : (
                            recentChunks.map((chunk, i) => (
                                <div key={chunk.id}>
                                    {i > 0 && <Separator />}
                                    <Link to="/chunks/$chunkId" params={{ chunkId: chunk.id }} className="block">
                                        <CardPanel className="hover:bg-muted/50 flex items-center justify-between gap-4 p-4 transition-colors">
                                            <div className="min-w-0">
                                                <p className="truncate text-sm font-medium">{chunk.title}</p>
                                                <div className="mt-1 flex items-center gap-2">
                                                    <Badge variant="secondary" size="sm" className="font-mono text-[10px]">
                                                        {chunk.type}
                                                    </Badge>
                                                    {(chunk.tags as string[]).map(tag => (
                                                        <Badge key={tag} variant="outline" size="sm" className="text-[10px]">
                                                            {tag}
                                                        </Badge>
                                                    ))}
                                                </div>
                                            </div>
                                            <span className="text-muted-foreground flex shrink-0 items-center gap-1 text-xs">
                                                <Clock className="size-3" />
                                                {new Date(chunk.updatedAt).toLocaleDateString()}
                                            </span>
                                        </CardPanel>
                                    </Link>
                                </div>
                            ))
                        )}
                    </Card>
                </div>

                <div>
                    <h2 className="mb-3 font-semibold">System</h2>
                    <Card>
                        <CardHeader>
                            <CardTitle className="text-sm">Status</CardTitle>
                            <CardDescription>Service health</CardDescription>
                        </CardHeader>
                        <CardPanel className="space-y-3 pt-0">
                            <div className="flex items-center justify-between text-sm">
                                <span>API Server</span>
                                <Badge variant={healthCheck.data ? "default" : "destructive"} size="sm">
                                    {healthCheck.isLoading ? "checking..." : healthCheck.data ? "online" : "offline"}
                                </Badge>
                            </div>
                            <Separator />
                            <div className="flex items-center justify-between text-sm">
                                <span>Database</span>
                                <Badge variant={healthCheck.data ? "default" : "destructive"} size="sm">
                                    {healthCheck.isLoading ? "checking..." : healthCheck.data ? "connected" : "disconnected"}
                                </Badge>
                            </div>
                            <Separator />
                            <div className="flex items-center justify-between text-sm">
                                <span>Auth</span>
                                <Badge variant={session ? "default" : "secondary"} size="sm">
                                    {session ? "authenticated" : "guest"}
                                </Badge>
                            </div>
                        </CardPanel>
                    </Card>
                </div>
            </div>
        </div>
    );
}
