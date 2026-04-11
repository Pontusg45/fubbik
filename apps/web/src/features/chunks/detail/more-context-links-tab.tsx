import { ArrowLeft, ArrowRight, Lightbulb } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { ChunkLink } from "@/features/chunks/chunk-link";
import { DeleteConnectionButton } from "@/features/chunks/delete-connection-button";
import { RelatedSuggestions } from "@/features/chunks/related-suggestions";
import { relationColor } from "@/features/chunks/relation-colors";
import { SuggestedConnections } from "@/features/chunks/suggested-connections";

export interface ConnectionItem {
    id: string;
    sourceId: string;
    targetId: string;
    relation: string;
    title?: string | null;
    codebaseName?: string | null;
}

export interface MoreContextLinksTabProps {
    chunkId: string;
    chunkTitle: string;
    outgoing: ConnectionItem[];
    incoming: ConnectionItem[];
}

export function MoreContextLinksTab({ chunkId, chunkTitle, outgoing, incoming }: MoreContextLinksTabProps) {
    const allConnections = [...outgoing, ...incoming];
    const connectedIds = allConnections.map(c => (c.sourceId === chunkId ? c.targetId : c.sourceId));
    const hasConnections = outgoing.length > 0 || incoming.length > 0;

    return (
        <div className="space-y-6 px-1 pb-4">
            {/* Connected — the source of truth */}
            <section>
                <div className="mb-2 flex items-center justify-between">
                    <h3 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                        Connected
                    </h3>
                    {hasConnections && (
                        <span className="text-[10px] text-muted-foreground font-mono">
                            {outgoing.length} out · {incoming.length} in
                        </span>
                    )}
                </div>

                {!hasConnections ? (
                    <p className="text-xs text-muted-foreground">No connections yet. Use suggestions below to discover links.</p>
                ) : (
                    <div className="space-y-1">
                        {outgoing.map(conn => (
                            <div key={conn.id} className="flex items-center gap-2 rounded border px-3 py-2 text-sm">
                                <span title="Outgoing">
                                    <ArrowRight className="size-3 shrink-0 text-muted-foreground/60" />
                                </span>
                                <Badge
                                    variant="outline"
                                    size="sm"
                                    style={{ borderColor: relationColor(conn.relation) }}
                                >
                                    {conn.relation}
                                </Badge>
                                <span className="flex-1 truncate">
                                    <ChunkLink chunkId={conn.targetId}>
                                        {conn.title ?? conn.targetId}
                                    </ChunkLink>
                                </span>
                                <DeleteConnectionButton connectionId={conn.id} chunkId={chunkId} />
                            </div>
                        ))}
                        {incoming.map(conn => (
                            <div key={conn.id} className="flex items-center gap-2 rounded border px-3 py-2 text-sm">
                                <span title="Incoming">
                                    <ArrowLeft className="size-3 shrink-0 text-muted-foreground/60" />
                                </span>
                                <Badge
                                    variant="outline"
                                    size="sm"
                                    style={{ borderColor: relationColor(conn.relation) }}
                                >
                                    {conn.relation}
                                </Badge>
                                <span className="flex-1 truncate">
                                    <ChunkLink chunkId={conn.sourceId}>
                                        {conn.title ?? conn.sourceId}
                                    </ChunkLink>
                                </span>
                            </div>
                        ))}
                    </div>
                )}
            </section>

            {/* Discover — unified suggestions */}
            <section>
                <h3 className="mb-2 flex items-center gap-1.5 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                    <Lightbulb className="size-3.5" />
                    Discover
                </h3>

                <div className="space-y-3">
                    <div>
                        <div className="mb-1 text-[10px] text-muted-foreground/70">By content similarity</div>
                        <RelatedSuggestions chunkId={chunkId} chunkTitle={chunkTitle} connectedIds={connectedIds} />
                    </div>

                    <div>
                        <div className="mb-1 text-[10px] text-muted-foreground/70">By rule-based analysis</div>
                        <SuggestedConnections chunkId={chunkId} />
                    </div>
                </div>
            </section>
        </div>
    );
}
