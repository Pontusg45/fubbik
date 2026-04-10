import { Link } from "@tanstack/react-router";
import { Badge } from "@/components/ui/badge";

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
        return (
            <div className="py-16 text-center text-muted-foreground text-sm">
                No chunks to show.
            </div>
        );
    }

    return (
        <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4">
            {chunks.map(chunk => (
                <Link
                    key={chunk.id}
                    to="/chunks/$chunkId"
                    params={{ chunkId: chunk.id }}
                    className="group flex h-full flex-col rounded-lg border bg-card p-4 hover:bg-muted/40 hover:border-foreground/20 transition-all"
                >
                    <div className="mb-2 flex items-start justify-between gap-2">
                        <span className="line-clamp-2 text-sm font-semibold leading-tight text-foreground group-hover:text-primary transition-colors">
                            {chunk.title}
                        </span>
                        <Badge variant="secondary" size="sm" className="shrink-0 font-mono text-[9px]">
                            {chunk.type}
                        </Badge>
                    </div>
                    {chunk.summary && (
                        <p className="mb-2 line-clamp-2 text-xs italic text-muted-foreground">
                            {chunk.summary}
                        </p>
                    )}
                    {chunk.content && (
                        <p className="line-clamp-4 text-xs text-muted-foreground/80 flex-1">
                            {chunk.content.slice(0, 200)}
                        </p>
                    )}
                    {chunk.tags && chunk.tags.length > 0 && (
                        <div className="mt-3 flex flex-wrap gap-1">
                            {chunk.tags.slice(0, 3).map((t, i) => {
                                const name = typeof t === "string" ? t : t.name;
                                return (
                                    <span
                                        key={`${name}-${i}`}
                                        className="rounded bg-muted px-1.5 py-0.5 text-[9px] text-muted-foreground"
                                    >
                                        {name}
                                    </span>
                                );
                            })}
                            {chunk.tags.length > 3 && (
                                <span className="text-[9px] text-muted-foreground">
                                    +{chunk.tags.length - 3}
                                </span>
                            )}
                        </div>
                    )}
                </Link>
            ))}
        </div>
    );
}
