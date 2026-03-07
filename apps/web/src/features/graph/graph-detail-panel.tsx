import { useQuery } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { Calendar, Clock, ExternalLink, FileText, Hash, Network, X } from "lucide-react";
import { MarkdownRenderer } from "@/components/markdown-renderer";

import { Badge } from "@/components/ui/badge";
import { Separator } from "@/components/ui/separator";
import { getChunkSize } from "@/features/chunks/chunk-size";
import { relationColor } from "@/features/chunks/relation-colors";
import { api } from "@/utils/api";

export function GraphDetailPanel({ chunkId, onClose, onNavigateToChunk }: {
    chunkId: string;
    onClose: () => void;
    onNavigateToChunk: (id: string) => void;
}) {
    const { data, isLoading } = useQuery({
        queryKey: ["chunk", chunkId],
        queryFn: async () => {
            const { data, error } = await api.api.chunks({ id: chunkId }).get();
            if (error) throw new Error("Failed to load chunk");
            return data;
        }
    });

    return (
        <div className="flex h-full flex-col md:border-r bg-background">
            {/* Header */}
            <div className="flex items-center justify-between border-b px-4 py-3">
                <span className="text-xs font-medium text-muted-foreground">Chunk Details</span>
                <div className="flex items-center gap-1">
                    <Link
                        to="/chunks/$chunkId"
                        params={{ chunkId }}
                        className="rounded-md p-1.5 text-muted-foreground hover:text-foreground hover:bg-muted transition-colors"
                        title="Open full page"
                    >
                        <ExternalLink className="size-3.5" />
                    </Link>
                    <button
                        onClick={onClose}
                        className="rounded-md p-1.5 text-muted-foreground hover:text-foreground hover:bg-muted transition-colors"
                    >
                        <X className="size-3.5" />
                    </button>
                </div>
            </div>

            {/* Content */}
            <div className="flex-1 overflow-y-auto px-4 py-4">
                {isLoading && <p className="text-muted-foreground text-sm">Loading...</p>}

                {data?.chunk && (() => {
                    const chunk = data.chunk;
                    const connections = data.connections ?? [];
                    const outgoing = connections.filter(c => c.sourceId === chunkId);
                    const incoming = connections.filter(c => c.sourceId !== chunkId);
                    const size = getChunkSize(chunk.content);

                    return (
                        <div className="space-y-4">
                            {/* Type & ID */}
                            <div className="flex items-center gap-2">
                                <Badge variant="secondary" className="font-mono text-xs">
                                    {chunk.type}
                                </Badge>
                                <span className="text-muted-foreground flex items-center gap-1 font-mono text-xs">
                                    <Hash className="size-3" />
                                    {chunk.id.slice(0, 8)}
                                </span>
                            </div>

                            {/* Title */}
                            <h2 className="text-lg font-bold tracking-tight">{chunk.title}</h2>

                            {/* Meta */}
                            <div className="text-muted-foreground flex flex-wrap items-center gap-3 text-xs">
                                <span className="flex items-center gap-1">
                                    <Calendar className="size-3" />
                                    {new Date(chunk.createdAt).toLocaleDateString()}
                                </span>
                                <span className="flex items-center gap-1">
                                    <Clock className="size-3" />
                                    {new Date(chunk.updatedAt).toLocaleDateString()}
                                </span>
                                <span className="flex items-center gap-1" style={{ color: size.color }}>
                                    <FileText className="size-3" />
                                    {size.lines}L · {size.chars.toLocaleString()}C
                                </span>
                            </div>

                            {/* Tags */}
                            {(chunk.tags as string[]).length > 0 && (
                                <div className="flex flex-wrap gap-1">
                                    {(chunk.tags as string[]).map(tag => (
                                        <Badge key={tag} variant="outline" size="sm">
                                            {tag}
                                        </Badge>
                                    ))}
                                </div>
                            )}

                            <Separator />

                            {/* Content */}
                            <div className="prose dark:prose-invert prose-sm max-w-none">
                                <MarkdownRenderer>{chunk.content}</MarkdownRenderer>
                            </div>

                            {/* Connections */}
                            {connections.length > 0 && (
                                <>
                                    <Separator />
                                    <div>
                                        <h3 className="flex items-center gap-2 text-sm font-medium mb-2">
                                            <Network className="size-3.5" />
                                            Connections ({connections.length})
                                        </h3>

                                        {outgoing.length > 0 && (
                                            <div className="mb-3">
                                                <p className="text-muted-foreground text-[10px] font-medium uppercase mb-1">Links to</p>
                                                <div className="space-y-1">
                                                    {outgoing.map(conn => (
                                                        <button
                                                            key={conn.id}
                                                            onClick={() => onNavigateToChunk(conn.targetId)}
                                                            className="flex w-full items-center justify-between rounded-md border px-2.5 py-1.5 text-xs hover:bg-muted transition-colors text-left"
                                                        >
                                                            <span className="truncate font-medium">{conn.title ?? conn.targetId.slice(0, 8)}</span>
                                                            <Badge variant="outline" size="sm" className="ml-2 shrink-0 text-[9px]" style={{ borderColor: relationColor(conn.relation), color: relationColor(conn.relation) }}>
                                                                {conn.relation}
                                                            </Badge>
                                                        </button>
                                                    ))}
                                                </div>
                                            </div>
                                        )}

                                        {incoming.length > 0 && (
                                            <div>
                                                <p className="text-muted-foreground text-[10px] font-medium uppercase mb-1">Linked from</p>
                                                <div className="space-y-1">
                                                    {incoming.map(conn => (
                                                        <button
                                                            key={conn.id}
                                                            onClick={() => onNavigateToChunk(conn.sourceId)}
                                                            className="flex w-full items-center justify-between rounded-md border px-2.5 py-1.5 text-xs hover:bg-muted transition-colors text-left"
                                                        >
                                                            <span className="truncate font-medium">{conn.title ?? conn.sourceId.slice(0, 8)}</span>
                                                            <Badge variant="outline" size="sm" className="ml-2 shrink-0 text-[9px]" style={{ borderColor: relationColor(conn.relation), color: relationColor(conn.relation) }}>
                                                                {conn.relation}
                                                            </Badge>
                                                        </button>
                                                    ))}
                                                </div>
                                            </div>
                                        )}
                                    </div>
                                </>
                            )}
                        </div>
                    );
                })()}
            </div>
        </div>
    );
}
