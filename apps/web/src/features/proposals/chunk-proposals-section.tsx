import { useQuery } from "@tanstack/react-query";
import { AlertTriangle } from "lucide-react";

import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

import { ProposalCard, type Proposal } from "./proposal-card";

export interface ChunkProposalsSectionProps {
    chunkId: string;
}

export function ChunkProposalsSection({ chunkId }: ChunkProposalsSectionProps) {
    const proposalsQuery = useQuery({
        queryKey: ["chunk-proposals", chunkId],
        queryFn: async () =>
            unwrapEden(await (api.api as any).chunks[chunkId].proposals.get({ query: { status: "pending" } })),
    });

    const proposals = (proposalsQuery.data ?? []) as Proposal[];
    if (proposals.length === 0) return null;

    return (
        <section className="space-y-2">
            <h3 className="flex items-center gap-1.5 text-xs font-semibold uppercase tracking-wider text-amber-500">
                <AlertTriangle className="size-3.5" />
                Pending proposals ({proposals.length})
            </h3>
            <div className="space-y-2">
                {proposals.map(p => (
                    <ProposalCard
                        key={p.id}
                        proposal={p}
                        showChunkInfo={false}
                        onUpdate={() => { void proposalsQuery.refetch(); }}
                    />
                ))}
            </div>
        </section>
    );
}
