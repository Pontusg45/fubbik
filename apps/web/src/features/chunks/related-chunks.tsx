import { useQuery } from "@tanstack/react-query";
import { ChevronDown, Link2, Tag } from "lucide-react";
import { useState } from "react";

import { Badge } from "@/components/ui/badge";
import { Collapsible, CollapsiblePanel, CollapsibleTrigger } from "@/components/ui/collapsible";
import { api } from "@/utils/api";

import { ChunkLink } from "./chunk-link";
import { relationColor } from "./relation-colors";

interface RelatedChunksProps {
    chunkId: string;
    connections: Array<{ id: string; sourceId: string; targetId: string; relation: string; title: string | null }>;
    tags: Array<{ id: string; name: string }>;
}

interface RelatedItem {
    id: string;
    title: string;
    source: "connection" | "shared-tag";
    relation?: string;
}

export function RelatedChunks({ chunkId, connections, tags }: RelatedChunksProps) {
    const [open, setOpen] = useState(true);

    const tagNames = tags.map(t => t.name);

    const { data: sharedTagChunks } = useQuery({
        queryKey: ["chunks", "shared-tags", chunkId, tagNames.join(",")],
        queryFn: async () => {
            if (tagNames.length === 0) return { chunks: [] };
            const { data, error } = await api.api.chunks.get({
                query: { tags: tagNames.join(","), limit: "5", exclude: chunkId }
            });
            if (error) return { chunks: [] };
            return data;
        },
        enabled: tagNames.length > 0,
        staleTime: 2 * 60 * 1000
    });

    // Build deduplicated list: connections first, then shared-tag chunks
    const items: RelatedItem[] = [];
    const seenIds = new Set<string>();

    for (const conn of connections) {
        const linkedId = conn.sourceId === chunkId ? conn.targetId : conn.sourceId;
        if (!seenIds.has(linkedId)) {
            seenIds.add(linkedId);
            items.push({
                id: linkedId,
                title: conn.title ?? linkedId.slice(0, 8),
                source: "connection",
                relation: conn.relation
            });
        }
    }

    if (sharedTagChunks?.chunks) {
        for (const c of sharedTagChunks.chunks) {
            if (!seenIds.has(c.id)) {
                seenIds.add(c.id);
                items.push({
                    id: c.id,
                    title: c.title,
                    source: "shared-tag"
                });
            }
        }
    }

    const displayItems = items.slice(0, 10);

    if (displayItems.length === 0) {
        return null;
    }

    return (
        <Collapsible open={open} onOpenChange={setOpen}>
            <CollapsibleTrigger className="flex w-full items-center justify-between py-2">
                <h2 className="flex items-center gap-2 text-sm font-semibold">
                    <Link2 className="size-4" />
                    Related Chunks
                    <Badge variant="secondary" size="sm" className="text-[10px]">
                        {displayItems.length}
                    </Badge>
                </h2>
                <ChevronDown
                    className={`text-muted-foreground size-4 transition-transform duration-200 ${open ? "rotate-180" : ""}`}
                />
            </CollapsibleTrigger>
            <CollapsiblePanel>
                <div className="space-y-1.5 pt-2">
                    {displayItems.map(item => (
                        <div
                            key={item.id}
                            className="hover:bg-muted flex items-center justify-between rounded-md border px-3 py-2 text-sm transition-colors"
                        >
                            <ChunkLink chunkId={item.id}>{item.title}</ChunkLink>
                            <div className="ml-2 shrink-0">
                                {item.source === "connection" && item.relation ? (
                                    <Badge
                                        variant="outline"
                                        size="sm"
                                        className="text-[10px]"
                                        style={{
                                            borderColor: relationColor(item.relation),
                                            color: relationColor(item.relation)
                                        }}
                                    >
                                        {item.relation}
                                    </Badge>
                                ) : (
                                    <Badge variant="outline" size="sm" className="text-[10px]">
                                        <Tag className="size-2.5" />
                                        shared tag
                                    </Badge>
                                )}
                            </div>
                        </div>
                    ))}
                </div>
            </CollapsiblePanel>
        </Collapsible>
    );
}
