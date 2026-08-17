import { useQuery } from "@tanstack/react-query";
import { AlertTriangle } from "lucide-react";

import { legacyApi } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

import { ProposalCard, type Proposal } from "./proposal-card";

export interface ChunkProposalsSectionProps {
    chunkId: string;
}

export function ChunkProposalsSection({ chunkId }: ChunkProposalsSectionProps) {
    const proposalsQuery = useQuery({
        queryKey: ["chunk-proposals", chunkId],
        // /chunks/:id/proposals is part of the Node-only proposals domain — must stay on legacyApi.
        // Bridge through `unknown`: Eden's inferred shape doesn't structurally overlap
        // the local `Proposal` shape used for rendering.
        queryFn: async () =>
            unwrapEden(
                await legacyApi.api.chunks({ id: chunkId }).proposals.get({ query: { status: "pending" } })
            ) as unknown as Proposal[]
    });

    const proposals = proposalsQuery.data ?? [];
    if (proposals.length === 0) return null;

    return (
        <section className="space-y-2">
            <h3 className="flex items-center gap-1.5 text-xs font-semibold tracking-wider text-amber-500 uppercase">
                <AlertTriangle className="size-3.5" />
                Pending proposals ({proposals.length})
            </h3>
            <div className="space-y-2">
                {proposals.map(p => (
                    <ProposalCard
                        key={p.id}
                        proposal={p}
                        showChunkInfo={false}
                        onUpdate={() => {
                            void proposalsQuery.refetch();
                        }}
                    />
                ))}
            </div>
        </section>
    );
}
