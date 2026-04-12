import { useMutation, useQuery } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { Check, ChevronDown, ChevronRight, X } from "lucide-react";
import { useState } from "react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

import { ProposalDiff } from "./proposal-diff";

export interface Proposal {
    id: string;
    chunkId: string;
    changes: Record<string, unknown>;
    reason: string | null;
    status: string;
    proposedBy: string;
    reviewedBy: string | null;
    reviewedAt: string | null;
    reviewNote: string | null;
    createdAt: string;
    chunkTitle?: string;
    chunkType?: string;
}

export interface ProposalCardProps {
    proposal: Proposal;
    showChunkInfo?: boolean;
    onUpdate: () => void;
}

export function ProposalCard({ proposal, showChunkInfo = true, onUpdate }: ProposalCardProps) {
    const [expanded, setExpanded] = useState(false);

    // Fetch chunk detail for diff rendering when expanded
    const chunkQuery = useQuery({
        queryKey: ["chunk-for-diff", proposal.chunkId],
        queryFn: async () => unwrapEden(await (api.api as any).chunks[proposal.chunkId].get()),
        enabled: expanded,
    });

    const approveMutation = useMutation({
        mutationFn: async () =>
            unwrapEden(await (api.api as any).proposals[proposal.id].approve.post({})),
        onSuccess: () => onUpdate(),
    });

    const rejectMutation = useMutation({
        mutationFn: async () =>
            unwrapEden(await (api.api as any).proposals[proposal.id].reject.post({})),
        onSuccess: () => onUpdate(),
    });

    const changedFields = Object.keys(proposal.changes);
    const isPending = proposal.status === "pending";
    const age = getRelativeTime(proposal.createdAt);

    return (
        <div className="rounded-md border bg-card">
            <div className="flex items-start gap-3 p-3">
                <button type="button" onClick={() => setExpanded(e => !e)} className="mt-0.5 text-muted-foreground">
                    {expanded ? <ChevronDown className="size-4" /> : <ChevronRight className="size-4" />}
                </button>
                <div className="flex-1">
                    {showChunkInfo && (
                        <div className="mb-1 flex items-center gap-2">
                            <Link
                                to="/chunks/$chunkId"
                                params={{ chunkId: proposal.chunkId }}
                                className="font-medium hover:underline"
                            >
                                {proposal.chunkTitle ?? proposal.chunkId.slice(0, 8)}
                            </Link>
                            {proposal.chunkType && (
                                <Badge variant="secondary" size="sm">{proposal.chunkType}</Badge>
                            )}
                        </div>
                    )}
                    <div className="flex items-center gap-2 text-xs text-muted-foreground">
                        <span>{age}</span>
                        {proposal.reason && (
                            <>
                                <span>•</span>
                                <span className="truncate max-w-[300px]">{proposal.reason}</span>
                            </>
                        )}
                    </div>
                    <div className="mt-1 flex flex-wrap gap-1">
                        {changedFields.map(f => (
                            <span key={f} className="rounded bg-muted px-1.5 py-0.5 text-[10px] font-mono">
                                {f}
                            </span>
                        ))}
                    </div>
                </div>
                {isPending && (
                    <div className="flex gap-1">
                        <Button
                            size="sm"
                            variant="ghost"
                            onClick={() => approveMutation.mutate()}
                            disabled={approveMutation.isPending}
                            title="Approve"
                        >
                            <Check className="size-4 text-emerald-500" />
                        </Button>
                        <Button
                            size="sm"
                            variant="ghost"
                            onClick={() => rejectMutation.mutate()}
                            disabled={rejectMutation.isPending}
                            title="Reject"
                        >
                            <X className="size-4 text-red-500" />
                        </Button>
                    </div>
                )}
                {!isPending && (
                    <Badge variant={proposal.status === "approved" ? "default" : "secondary"} size="sm">
                        {proposal.status}
                    </Badge>
                )}
            </div>
            {expanded && (
                <div className="border-t px-3 py-3">
                    {chunkQuery.isLoading ? (
                        <div className="text-xs text-muted-foreground">Loading chunk...</div>
                    ) : chunkQuery.data ? (
                        <ProposalDiff
                            currentChunk={(chunkQuery.data as any).chunk ?? chunkQuery.data}
                            changes={proposal.changes}
                        />
                    ) : (
                        <div className="text-xs text-muted-foreground">Could not load chunk data</div>
                    )}
                </div>
            )}
        </div>
    );
}

function getRelativeTime(dateStr: string): string {
    const diff = Date.now() - new Date(dateStr).getTime();
    const mins = Math.floor(diff / 60000);
    if (mins < 60) return `${mins}m ago`;
    const hours = Math.floor(mins / 60);
    if (hours < 24) return `${hours}h ago`;
    const days = Math.floor(hours / 24);
    return `${days}d ago`;
}
