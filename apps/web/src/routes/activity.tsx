import { useQuery } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import { Blocks, ClipboardCheck, FolderGit2, Link2, Loader2, Network, Tags } from "lucide-react";
import { useState } from "react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardPanel } from "@/components/ui/card";
import { Separator } from "@/components/ui/separator";
import { useActiveCodebase } from "@/features/codebases/use-active-codebase";
import { getUser } from "@/functions/get-user";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export const Route = createFileRoute("/activity")({
    component: ActivityPage,
    beforeLoad: async () => {
        let session = null;
        try {
            session = await getUser();
        } catch {
            // allow guest access
        }
        return { session };
    }
});

const ENTITY_TYPES = [
    { value: "", label: "All" },
    { value: "chunk", label: "Chunks" },
    { value: "requirement", label: "Requirements" },
    { value: "connection", label: "Connections" },
    { value: "tag", label: "Tags" },
    { value: "codebase", label: "Codebases" }
];

function entityIcon(entityType: string) {
    switch (entityType) {
        case "chunk":
            return Blocks;
        case "requirement":
            return ClipboardCheck;
        case "connection":
            return Link2;
        case "tag":
            return Tags;
        case "codebase":
            return FolderGit2;
        default:
            return Network;
    }
}

function actionColor(action: string): "success" | "info" | "destructive" | "warning" | "secondary" | "outline" {
    switch (action) {
        case "created":
            return "success";
        case "updated":
            return "info";
        case "deleted":
            return "destructive";
        case "archived":
            return "warning";
        case "restored":
            return "secondary";
        default:
            return "outline";
    }
}

function timeAgo(date: string | Date) {
    const now = Date.now();
    const then = new Date(date).getTime();
    const seconds = Math.floor((now - then) / 1000);
    if (seconds < 60) return "just now";
    const minutes = Math.floor(seconds / 60);
    if (minutes < 60) return `${minutes}m ago`;
    const hours = Math.floor(minutes / 60);
    if (hours < 24) return `${hours}h ago`;
    const days = Math.floor(hours / 24);
    if (days < 30) return `${days}d ago`;
    return new Date(date).toLocaleDateString();
}

function ActivityPage() {
    const { codebaseId } = useActiveCodebase();
    const [entityTypeFilter, setEntityTypeFilter] = useState("");

    const activityQuery = useQuery({
        queryKey: ["activity", codebaseId, entityTypeFilter],
        queryFn: async () => {
            const query: Record<string, string> = { limit: "50" };
            if (codebaseId) query.codebaseId = codebaseId;
            if (entityTypeFilter) query.entityType = entityTypeFilter;
            return unwrapEden(await api.api.activity.get({ query }));
        }
    });

    const activities = activityQuery.data ?? [];

    return (
        <div className="container mx-auto max-w-3xl px-4 py-8">
            <div className="mb-6">
                <h1 className="text-2xl font-bold tracking-tight">Activity</h1>
                <p className="text-muted-foreground text-sm">Recent changes across your knowledge base.</p>
            </div>

            <div className="mb-4 flex flex-wrap gap-1">
                {ENTITY_TYPES.map(t => (
                    <Button
                        key={t.value}
                        variant={entityTypeFilter === t.value ? "default" : "outline"}
                        size="xs"
                        onClick={() => setEntityTypeFilter(t.value)}
                    >
                        {t.label}
                    </Button>
                ))}
            </div>

            <Card>
                {activityQuery.isLoading ? (
                    <CardPanel className="flex items-center justify-center p-8">
                        <Loader2 className="text-muted-foreground size-5 animate-spin" />
                    </CardPanel>
                ) : activities.length === 0 ? (
                    <CardPanel className="p-8 text-center">
                        <p className="text-muted-foreground text-sm">No activity yet.</p>
                    </CardPanel>
                ) : (
                    activities.map((entry, i) => {
                        const Icon = entityIcon(entry.entityType);
                        return (
                            <div key={entry.id}>
                                {i > 0 && <Separator />}
                                <CardPanel className="flex items-center gap-3 p-4">
                                    <div className="bg-muted rounded-md p-2">
                                        <Icon className="text-muted-foreground size-4" />
                                    </div>
                                    <div className="min-w-0 flex-1">
                                        <p className="text-sm">
                                            <Badge variant={actionColor(entry.action)} size="sm" className="mr-1.5">
                                                {entry.action}
                                            </Badge>
                                            <span className="font-medium">{entry.entityTitle ?? entry.entityId}</span>
                                        </p>
                                        <p className="text-muted-foreground mt-0.5 text-xs">
                                            {entry.entityType} &middot; {timeAgo(entry.createdAt)}
                                        </p>
                                    </div>
                                </CardPanel>
                            </div>
                        );
                    })
                )}
            </Card>
        </div>
    );
}
