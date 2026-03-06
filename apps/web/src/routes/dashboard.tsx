import { useQuery } from "@tanstack/react-query";
import { createFileRoute, Link } from "@tanstack/react-router";
import { Blocks, Clock, Network, Plus, Tags } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardDescription, CardHeader, CardPanel, CardTitle } from "@/components/ui/card";
import { Separator } from "@/components/ui/separator";
import { getUser } from "@/functions/get-user";
import { api } from "@/utils/api";

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

    const healthCheck = useQuery({
        queryKey: ["health"],
        queryFn: async () => {
            const { data } = await api.api.health.get();
            return data;
        }
    });

    const statsQuery = useQuery({
        queryKey: ["stats"],
        queryFn: async () => {
            const { data, error } = await api.api.stats.get();
            if (error) return null;
            return data;
        }
    });

    const chunksQuery = useQuery({
        queryKey: ["chunks"],
        queryFn: async () => {
            const { data, error } = await api.api.chunks.get({ query: { limit: "5" } });
            if (error) return null;
            return data;
        }
    });

    const stats = [
        { label: "Chunks", value: statsQuery.data?.chunks ?? 0, icon: Blocks },
        { label: "Connections", value: statsQuery.data?.connections ?? 0, icon: Network },
        { label: "Tags", value: statsQuery.data?.tags ?? 0, icon: Tags }
    ];

    const recentChunks = chunksQuery.data?.chunks ?? [];

    return (
        <div className="container mx-auto max-w-5xl px-4 py-8">
            <div className="mb-8 flex items-center justify-between">
                <div>
                    <h1 className="text-2xl font-bold tracking-tight">Welcome back{session?.user.name ? `, ${session.user.name}` : ""}</h1>
                    <p className="text-muted-foreground text-sm">Here's an overview of your knowledge base.</p>
                </div>
                <Button render={<Link to="/chunks/new" />}>
                    <Plus className="size-4" />
                    New Chunk
                </Button>
            </div>

            <div className="mb-8 grid grid-cols-2 gap-4 sm:grid-cols-3">
                {stats.map(stat => (
                    <Card key={stat.label}>
                        <CardPanel className="flex items-center gap-3 p-4">
                            <div className="bg-muted rounded-md p-2">
                                <stat.icon className="text-muted-foreground size-4" />
                            </div>
                            <div>
                                <p className="text-2xl font-bold">{statsQuery.isLoading ? "—" : stat.value}</p>
                                <p className="text-muted-foreground text-xs">{stat.label}</p>
                            </div>
                        </CardPanel>
                    </Card>
                ))}
            </div>

            <div className="grid gap-6 lg:grid-cols-3">
                <div className="lg:col-span-2">
                    <div className="mb-3 flex items-center justify-between">
                        <h2 className="font-semibold">Recent Chunks</h2>
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
