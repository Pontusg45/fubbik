import { useQuery } from "@tanstack/react-query";
import { createFileRoute, Link } from "@tanstack/react-router";
import { Clock, FilePlus2, History, Loader2 } from "lucide-react";
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

export const Route = createFileRoute("/timeline")({
    component: TimelinePage,
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

const RANGES = [
    { value: "7d", label: "7 days" },
    { value: "30d", label: "30 days" },
    { value: "90d", label: "90 days" },
    { value: "1y", label: "1 year" }
] as const;

type TimelineEvent = {
    chunkId: string;
    chunkTitle: string;
    chunkType: string;
    kind: "created" | "updated";
    at: string;
    version: number | null;
};

function dayBucket(at: string): string {
    const d = new Date(at.replace(" ", "T") + (at.includes("Z") ? "" : "Z"));
    return d.toISOString().slice(0, 10);
}

function formatDay(bucket: string): string {
    const d = new Date(bucket + "T00:00:00Z");
    return d.toLocaleDateString(undefined, { year: "numeric", month: "short", day: "numeric" });
}

function TimelinePage() {
    const { codebaseId } = useActiveCodebase();
    const [range, setRange] = useState<string>("30d");

    const timelineQuery = useQuery({
        queryKey: ["timeline", range, codebaseId],
        queryFn: async () => {
            const query: Record<string, string> = { range };
            if (codebaseId) query.codebaseId = codebaseId;
            return unwrapEden(await api.api.timeline.get({ query }));
        }
    });

    const data = timelineQuery.data;
    const events = (data?.events ?? []) as TimelineEvent[];
    const totals = data?.totals;

    const buckets = new Map<string, TimelineEvent[]>();
    for (const event of events) {
        const key = dayBucket(event.at);
        const existing = buckets.get(key) ?? [];
        existing.push(event);
        buckets.set(key, existing);
    }
    const sortedBuckets = Array.from(buckets.entries()).sort((a, b) => (a[0] < b[0] ? 1 : -1));

    return (
        <PageContainer maxWidth="4xl">
            <PageHeader
                icon={History}
                title="Timeline"
                description="How your knowledge base evolved over time — creations and updates grouped by day."
            />

            <div className="mb-4 flex flex-wrap gap-1">
                {RANGES.map(r => (
                    <Button
                        key={r.value}
                        variant={range === r.value ? "default" : "outline"}
                        size="xs"
                        onClick={() => setRange(r.value)}
                    >
                        {r.label}
                    </Button>
                ))}
            </div>

            {totals && (
                <Card className="mb-4">
                    <CardPanel className="flex items-center gap-6 p-4 text-sm">
                        <div className="flex items-center gap-2">
                            <FilePlus2 className="text-muted-foreground size-4" />
                            <span className="text-muted-foreground">Created</span>
                            <span className="font-semibold">{totals.created}</span>
                        </div>
                        <div className="flex items-center gap-2">
                            <History className="text-muted-foreground size-4" />
                            <span className="text-muted-foreground">Updated</span>
                            <span className="font-semibold">{totals.updated}</span>
                        </div>
                    </CardPanel>
                </Card>
            )}

            <Card>
                {timelineQuery.isLoading ? (
                    <CardPanel className="flex items-center justify-center p-8">
                        <Loader2 className="text-muted-foreground size-5 animate-spin" />
                    </CardPanel>
                ) : events.length === 0 ? (
                    <CardPanel className="p-8 text-center">
                        <p className="text-muted-foreground text-sm">No events in this range.</p>
                    </CardPanel>
                ) : (
                    sortedBuckets.map(([day, dayEvents], bucketIdx) => (
                        <div key={day}>
                            {bucketIdx > 0 && <Separator />}
                            <CardPanel className="p-4">
                                <div className="mb-3 flex items-center gap-2">
                                    <Clock className="text-muted-foreground size-3.5" />
                                    <span className="text-muted-foreground text-xs font-medium uppercase tracking-wide">
                                        {formatDay(day)}
                                    </span>
                                    <span className="text-muted-foreground text-xs">({dayEvents.length})</span>
                                </div>
                                <ul className="space-y-1.5">
                                    {dayEvents.map((event, i) => (
                                        <li key={`${event.chunkId}-${event.kind}-${event.version ?? i}`}>
                                            <Link
                                                to="/chunks/$chunkId"
                                                params={{ chunkId: event.chunkId }}
                                                className="hover:bg-muted/60 flex items-center gap-2 rounded px-1.5 py-1 text-sm"
                                            >
                                                <Badge
                                                    variant={event.kind === "created" ? "success" : "info"}
                                                    size="sm"
                                                >
                                                    {event.kind}
                                                    {event.version != null ? ` v${event.version}` : ""}
                                                </Badge>
                                                <span className="truncate font-medium">{event.chunkTitle}</span>
                                                <span className="text-muted-foreground ml-auto shrink-0 text-xs">
                                                    {event.chunkType}
                                                </span>
                                            </Link>
                                        </li>
                                    ))}
                                </ul>
                            </CardPanel>
                        </div>
                    ))
                )}
            </Card>
        </PageContainer>
    );
}
