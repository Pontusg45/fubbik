import { useQuery } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import { Blocks, ClipboardCheck, FolderGit2, Link2, Loader2, Network, Tags } from "lucide-react";
import { useState } from "react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardPanel } from "@/components/ui/card";
import { PageContainer, PageHeader } from "@/components/ui/page";
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

const ACTION_TYPES = [
    { value: "", label: "All" },
    { value: "created", label: "Created" },
    { value: "updated", label: "Updated" },
    { value: "deleted", label: "Deleted" },
    { value: "archived", label: "Archived" }
];

function ActivityPage() {
    const { codebaseId } = useActiveCodebase();
    const [entityTypeFilter, setEntityTypeFilter] = useState("");
    const [actionFilter, setActionFilter] = useState("");
    const [displayCount, setDisplayCount] = useState(20);

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
    const filtered = activities.filter((e: Record<string, unknown>) => !actionFilter || e.action === actionFilter);
    const displayed = filtered.slice(0, displayCount);

    return (
        <PageContainer>
            <PageHeader
                title="Activity"
                description="Recent changes across your knowledge base."
            />

            <div className="mb-4 space-y-2">
                <div className="flex flex-wrap gap-1">
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
                <div className="flex flex-wrap gap-1">
                    {ACTION_TYPES.map(a => (
                        <Button
                            key={a.value}
                            variant={actionFilter === a.value ? "default" : "outline"}
                            size="xs"
                            onClick={() => { setActionFilter(a.value); setDisplayCount(20); }}
                        >
                            {a.label}
                        </Button>
                    ))}
                </div>
            </div>

            <Card>
                {activityQuery.isLoading ? (
                    <CardPanel className="flex items-center justify-center p-8">
                        <Loader2 className="text-muted-foreground size-5 animate-spin" />
                    </CardPanel>
                ) : filtered.length === 0 ? (
                    <CardPanel className="p-8 text-center">
                        <p className="text-muted-foreground text-sm">No activity yet.</p>
                    </CardPanel>
                ) : (
                    displayed.map((entry: Record<string, any>, i: number) => {
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

            {displayCount < filtered.length && (
                <div className="mt-4 flex justify-center">
                    <Button variant="outline" size="sm" onClick={() => setDisplayCount(c => c + 20)}>
                        Load more ({filtered.length - displayCount} remaining)
                    </Button>
                </div>
            )}
        </PageContainer>
    );
}
