import { Link } from "@tanstack/react-router";

import { Badge } from "@/components/ui/badge";

import { ChunkTypeIcon } from "./chunk-type-icon";
import { ContentThumbnail } from "./content-thumbnail";

interface ChunkCardData {
    id: string;
    title: string;
    type: string;
    summary?: string | null;
    content?: string | null;
    tags?: Array<{ name: string } | string>;
}

export function ChunkCardGrid({ chunks }: { chunks: ChunkCardData[] }) {
    if (chunks.length === 0) {
        return <div className="text-muted-foreground py-16 text-center text-sm">No chunks to show.</div>;
    }

    return (
        <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4">
            {chunks.map(chunk => (
                <Link
                    key={chunk.id}
                    to="/chunks/$chunkId"
                    params={{ chunkId: chunk.id }}
                    className="group bg-card hover:bg-muted/40 hover:border-foreground/20 flex h-full flex-col rounded-lg border p-4 transition-all"
                >
                    {chunk.content && (
                        <div className="text-muted-foreground mb-3">
                            <ContentThumbnail content={chunk.content} className="h-10 w-full" />
                        </div>
                    )}
                    <div className="mb-2 flex items-start justify-between gap-2">
                        <span className="text-foreground group-hover:text-primary line-clamp-2 text-sm leading-tight font-semibold transition-colors">
                            {chunk.title}
                        </span>
                        <Badge variant="secondary" size="sm" className="flex shrink-0 items-center gap-1 font-mono text-[9px]">
                            <ChunkTypeIcon type={chunk.type} className="size-3" />
                            {chunk.type}
                        </Badge>
                    </div>
                    {chunk.summary && <p className="text-muted-foreground mb-2 line-clamp-2 text-xs italic">{chunk.summary}</p>}
                    {chunk.content && <p className="text-muted-foreground/80 line-clamp-4 flex-1 text-xs">{chunk.content.slice(0, 200)}</p>}
                    {chunk.tags && chunk.tags.length > 0 && (
                        <div className="mt-3 flex flex-wrap gap-1">
                            {chunk.tags.slice(0, 3).map((t, i) => {
                                const name = typeof t === "string" ? t : t.name;
                                return (
                                    <span key={`${name}-${i}`} className="bg-muted text-muted-foreground rounded px-1.5 py-0.5 text-[9px]">
                                        {name}
                                    </span>
                                );
                            })}
                            {chunk.tags.length > 3 && <span className="text-muted-foreground text-[9px]">+{chunk.tags.length - 3}</span>}
                        </div>
                    )}
                </Link>
            ))}
        </div>
    );
}
