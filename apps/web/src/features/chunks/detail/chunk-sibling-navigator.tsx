import { useQuery } from "@tanstack/react-query";
import { Link, useNavigate } from "@tanstack/react-router";
import { ChevronLeft, ChevronRight } from "lucide-react";
import { useEffect, useMemo } from "react";

import { Button } from "@/components/ui/button";
import { api } from "@/utils/api";

export interface ChunkSiblingNavigatorProps {
    currentChunkId: string;
    codebaseId?: string;
    codebaseName?: string;
}

interface SiblingChunk {
    id: string;
    title: string;
    type: string;
}

const WINDOW_SIZE = 10;

export function ChunkSiblingNavigator({
    currentChunkId,
    codebaseId,
    codebaseName,
}: ChunkSiblingNavigatorProps) {
    const navigate = useNavigate();

    const { data } = useQuery({
        queryKey: ["chunk-siblings", codebaseId],
        queryFn: async () => {
            const { data, error } = await api.api.chunks.get({
                query: {
                    ...(codebaseId ? { codebaseId } : {}),
                    sort: "updated",
                    limit: "50",
                } as any,
            });
            if (error) return { chunks: [] as SiblingChunk[], total: 0 };
            return data as unknown as { chunks: SiblingChunk[]; total: number };
        },
        enabled: !!codebaseId,
    });

    const allSiblings: SiblingChunk[] = useMemo(
        () => (data?.chunks ?? []).filter((c: SiblingChunk) => c.id !== currentChunkId),
        [data?.chunks, currentChunkId],
    );

    const total = allSiblings.length;
    const windowed = allSiblings.slice(0, WINDOW_SIZE);
    const prev = windowed[0];
    const next = windowed[1];

    useEffect(() => {
        function handleKey(e: KeyboardEvent) {
            const tag = document.activeElement?.tagName;
            if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return;
            if (e.metaKey || e.ctrlKey || e.altKey) return;
            if (e.key === "h" && prev) {
                e.preventDefault();
                void navigate({ to: "/chunks/$chunkId", params: { chunkId: prev.id } });
            } else if (e.key === "l" && next) {
                e.preventDefault();
                void navigate({ to: "/chunks/$chunkId", params: { chunkId: next.id } });
            }
        }
        window.addEventListener("keydown", handleKey);
        return () => window.removeEventListener("keydown", handleKey);
    }, [prev, next, navigate]);

    if (!codebaseId || total === 0) {
        return null;
    }

    return (
        <aside
            className="hidden xl:block w-[180px] shrink-0 print:hidden"
            data-focus-hide="true"
        >
            <div className="sticky top-8">
                <div className="text-[9px] font-semibold uppercase tracking-wider text-muted-foreground mb-2">
                    In this codebase
                </div>
                {codebaseName && (
                    <Link
                        to="/codebases/$codebaseId"
                        params={{ codebaseId }}
                        className="text-xs font-medium hover:text-primary transition-colors mb-3 block truncate"
                    >
                        {codebaseName}
                    </Link>
                )}
                <nav className="flex flex-col gap-0.5">
                    {windowed.map(sibling => (
                        <Link
                            key={sibling.id}
                            to="/chunks/$chunkId"
                            params={{ chunkId: sibling.id }}
                            className="rounded px-2 py-1.5 text-[11px] text-muted-foreground hover:bg-muted hover:text-foreground transition-colors truncate"
                            title={sibling.title}
                        >
                            {sibling.title}
                        </Link>
                    ))}
                </nav>
                {total > WINDOW_SIZE && (
                    <Link
                        to="/chunks"
                        search={{ codebaseId } as any}
                        className="mt-2 block text-[10px] text-muted-foreground hover:text-foreground transition-colors px-2"
                    >
                        See all ({total}) →
                    </Link>
                )}
                <div className="mt-3 flex gap-1 border-t pt-3">
                    <Button
                        variant="ghost"
                        size="sm"
                        disabled={!prev}
                        onClick={() => prev && void navigate({ to: "/chunks/$chunkId", params: { chunkId: prev.id } })}
                        className="flex-1 gap-1 text-[10px]"
                        title="Previous (h)"
                    >
                        <ChevronLeft className="size-3" />
                        Prev
                    </Button>
                    <Button
                        variant="ghost"
                        size="sm"
                        disabled={!next}
                        onClick={() => next && void navigate({ to: "/chunks/$chunkId", params: { chunkId: next.id } })}
                        className="flex-1 gap-1 text-[10px]"
                        title="Next (l)"
                    >
                        Next
                        <ChevronRight className="size-3" />
                    </Button>
                </div>
            </div>
        </aside>
    );
}
