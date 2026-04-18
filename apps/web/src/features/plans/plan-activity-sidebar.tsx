import { useQuery } from "@tanstack/react-query";

import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

interface ActivityRow {
    id: string;
    entityType: string;
    entityId: string;
    entityTitle: string | null;
    action: string;
    createdAt: string;
}

const ACTION_LABEL: Record<string, string> = {
    created: "created",
    updated: "edited",
    status_changed: "changed status",
    deleted: "deleted",
    duplicated: "duplicated",
};

function relativeTime(iso: string): string {
    const diff = Date.now() - new Date(iso).getTime();
    const m = Math.floor(diff / 60_000);
    if (m < 1) return "just now";
    if (m < 60) return `${m}m ago`;
    const h = Math.floor(m / 60);
    if (h < 24) return `${h}h ago`;
    const d = Math.floor(h / 24);
    if (d < 30) return `${d}d ago`;
    return new Date(iso).toLocaleDateString();
}

function dayKey(iso: string): string {
    const d = new Date(iso);
    return d.toISOString().slice(0, 10);
}

export function PlanActivitySidebar({ planId }: { planId: string }) {
    const activityQuery = useQuery({
        queryKey: ["plan-activity", planId],
        queryFn: async () => {
            try {
                return ((unwrapEden(await (api.api as any).plans[planId].activity.get())) as ActivityRow[]) ?? [];
            } catch {
                return [];
            }
        },
        staleTime: 10_000,
    });

    const events = activityQuery.data ?? [];

    // Group by day for visual rhythm.
    const groups: Array<{ day: string; events: ActivityRow[] }> = [];
    for (const e of events) {
        const day = dayKey(e.createdAt);
        const last = groups[groups.length - 1];
        if (last && last.day === day) last.events.push(e);
        else groups.push({ day, events: [e] });
    }

    return (
        <aside className="hidden w-64 shrink-0 lg:block">
            <div className="sticky top-20 space-y-3">
                <h3 className="text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
                    Activity
                </h3>
                {activityQuery.isLoading && (
                    <p className="text-muted-foreground text-xs">Loading…</p>
                )}
                {!activityQuery.isLoading && events.length === 0 && (
                    <p className="text-muted-foreground text-xs">No activity yet.</p>
                )}
                <div className="max-h-[calc(100vh-180px)] space-y-3 overflow-y-auto pr-1">
                    {groups.map(g => (
                        <div key={g.day}>
                            <div className="text-muted-foreground/70 mb-1 text-[10px] font-medium uppercase tracking-wide">
                                {new Date(g.day).toLocaleDateString(undefined, {
                                    month: "short",
                                    day: "numeric",
                                })}
                            </div>
                            <ul className="space-y-1.5 border-l border-muted pl-3">
                                {g.events.map(e => (
                                    <li key={e.id} className="text-xs leading-snug">
                                        <span className="text-muted-foreground">
                                            {e.entityType === "plan" ? "Plan" : "Task"}{" "}
                                            {ACTION_LABEL[e.action] ?? e.action}
                                        </span>
                                        {e.entityTitle && (
                                            <span className="block text-foreground">
                                                {e.entityTitle}
                                            </span>
                                        )}
                                        <span className="text-muted-foreground/70 text-[10px]">
                                            {relativeTime(e.createdAt)}
                                        </span>
                                    </li>
                                ))}
                            </ul>
                        </div>
                    ))}
                </div>
            </div>
        </aside>
    );
}
