import { useQuery } from "@tanstack/react-query";
import { Bot, MessageSquareText, Users } from "lucide-react";
import { useEffect, useState } from "react";

import { Badge } from "@/components/ui/badge";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export interface CoordinationRun {
    id: string;
    parentRunId: string | null;
    handle: string;
    status: string;
}

export interface CoordinationClaim {
    taskId: string;
    agentRunId: string;
    leaseExpiresAt: string;
    expired: boolean;
}

export interface CoordinationEntry {
    id: string;
    sequence: number;
    taskId: string | null;
    authorRunId: string;
    recipientRunId: string | null;
    kind: string;
    body: string;
    createdAt: string;
}

export interface CoordinationBoard {
    runs: CoordinationRun[];
    claims: CoordinationClaim[];
    entries: CoordinationEntry[];
    cursor: { nextSequence: number; hasMore: boolean };
}

export function usePlanCoordination(planId: string) {
    const [visible, setVisible] = useState(() => typeof document === "undefined" || document.visibilityState === "visible");
    const query = useQuery({
        queryKey: ["plan-coordination", planId],
        queryFn: async () => unwrapEden(await api.api.plans({ planId }).board.get()) as CoordinationBoard,
        refetchInterval: visible ? 5_000 : false,
        staleTime: 2_000
    });
    const { refetch } = query;

    useEffect(() => {
        const update = () => {
            const next = document.visibilityState === "visible";
            setVisible(next);
            if (next) void refetch();
        };
        document.addEventListener("visibilitychange", update);
        return () => document.removeEventListener("visibilitychange", update);
    }, [refetch]);

    return query;
}

export function PlanCoordinationPanel({ board, isLoading }: { board?: CoordinationBoard; isLoading: boolean }) {
    const runs = board?.runs ?? [];
    const entries = board?.entries ?? [];
    const runById = new Map(runs.map(run => [run.id, run]));

    return (
        <div className="space-y-4 border-b pb-4">
            <div>
                <h3 className="text-muted-foreground flex items-center gap-1.5 text-[10px] font-semibold tracking-wider uppercase">
                    <Users className="size-3" /> Agents
                </h3>
                {isLoading && <p className="text-muted-foreground mt-2 text-xs">Loading…</p>}
                {!isLoading && runs.length === 0 && <p className="text-muted-foreground mt-2 text-xs">No agent runs yet.</p>}
                <ul className="mt-2 space-y-1.5">
                    {runs.map(run => (
                        <li key={run.id} className="flex items-center gap-2 text-xs">
                            <Bot className="text-muted-foreground size-3" />
                            <span className="min-w-0 flex-1 truncate">{run.handle}</span>
                            <Badge size="sm" variant={run.status === "active" ? "success" : "secondary"}>
                                {run.status}
                            </Badge>
                        </li>
                    ))}
                </ul>
            </div>

            <div>
                <h3 className="text-muted-foreground flex items-center gap-1.5 text-[10px] font-semibold tracking-wider uppercase">
                    <MessageSquareText className="size-3" /> Journal
                </h3>
                {entries.length === 0 && !isLoading && <p className="text-muted-foreground mt-2 text-xs">No journal entries yet.</p>}
                <ol className="mt-2 max-h-64 space-y-2 overflow-y-auto pr-1">
                    {entries.map(entry => (
                        <li key={entry.id} className="border-muted border-l pl-2 text-xs">
                            <div className="text-muted-foreground flex items-center gap-1 text-[10px]">
                                <span>#{entry.sequence}</span>
                                <span>{entry.kind}</span>
                                <span>·</span>
                                <span>{runById.get(entry.authorRunId)?.handle ?? entry.authorRunId}</span>
                                {entry.recipientRunId && <span>→ {runById.get(entry.recipientRunId)?.handle ?? entry.recipientRunId}</span>}
                            </div>
                            <p className="mt-0.5 whitespace-pre-wrap">{entry.body}</p>
                        </li>
                    ))}
                </ol>
                {board?.cursor.hasMore && <p className="text-muted-foreground mt-2 text-[10px]">More journal entries are available.</p>}
            </div>
        </div>
    );
}
