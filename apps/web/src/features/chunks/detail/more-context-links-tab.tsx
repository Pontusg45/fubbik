import { Badge } from "@/components/ui/badge";
import { ChunkLink } from "@/features/chunks/chunk-link";
import { DeleteConnectionButton } from "@/features/chunks/delete-connection-button";
import { DependencyTree } from "@/features/chunks/dependency-tree";
import { RelatedChunks } from "@/features/chunks/related-chunks";
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

export interface TagItem {
    id: string;
    name: string;
}

export interface MoreContextLinksTabProps {
    chunkId: string;
    chunkTitle: string;
    outgoing: ConnectionItem[];
    incoming: ConnectionItem[];
    tags: TagItem[];
}

export function MoreContextLinksTab({ chunkId, chunkTitle, outgoing, incoming, tags }: MoreContextLinksTabProps) {
    const allConnections = [...outgoing, ...incoming];
    const connectedIds = allConnections.map(c => (c.sourceId === chunkId ? c.targetId : c.sourceId));

    return (
        <div className="space-y-6 px-1 pb-4">
            {outgoing.length > 0 && (
                <section>
                    <h3 className="mb-2 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                        Outgoing ({outgoing.length})
                    </h3>
                    <div className="space-y-1">
                        {outgoing.map(conn => (
                            <div key={conn.id} className="flex items-center gap-2 rounded border px-3 py-2 text-sm">
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
                    </div>
                </section>
            )}

            {incoming.length > 0 && (
                <section>
                    <h3 className="mb-2 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                        Incoming ({incoming.length})
                    </h3>
                    <div className="space-y-1">
                        {incoming.map(conn => (
                            <div key={conn.id} className="flex items-center gap-2 rounded border px-3 py-2 text-sm">
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
                </section>
            )}

            {outgoing.length === 0 && incoming.length === 0 && (
                <p className="text-xs text-muted-foreground">No connections yet.</p>
            )}

            <section>
                <h3 className="mb-2 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                    Dependency tree
                </h3>
                <DependencyTree chunkId={chunkId} connections={allConnections} />
            </section>

            <section>
                <h3 className="mb-2 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                    Suggested connections
                </h3>
                <SuggestedConnections chunkId={chunkId} />
            </section>

            <section>
                <h3 className="mb-2 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                    Related chunks
                </h3>
                <RelatedChunks
                    chunkId={chunkId}
                    connections={allConnections.map(c => ({ ...c, title: c.title ?? null }))}
                    tags={tags}
                />
            </section>

            <section>
                <h3 className="mb-2 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                    Semantic suggestions
                </h3>
                <RelatedSuggestions chunkId={chunkId} chunkTitle={chunkTitle} connectedIds={connectedIds} />
            </section>
        </div>
    );
}
