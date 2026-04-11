import { useQuery } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { Sparkles } from "lucide-react";
import { useMemo } from "react";
import { Badge } from "@/components/ui/badge";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export function FeaturedChunkWidget() {
    const { data } = useQuery({
        queryKey: ["featured-chunks"],
        queryFn: async () => unwrapEden(await api.api.chunks.get({ query: { limit: "100" } as any })),
        staleTime: 60 * 60 * 1000, // 1 hour
    });

    const chunks = ((data as any)?.chunks ?? []) as Array<{ id: string; title: string; type: string; summary?: string | null }>;

    // Deterministic daily selection using date hash
    const featured = useMemo(() => {
        if (chunks.length === 0) return null;
        const today = new Date().toISOString().slice(0, 10); // YYYY-MM-DD
        const hash = today.split("").reduce((acc, ch) => acc + ch.charCodeAt(0), 0);
        return chunks[hash % chunks.length];
    }, [chunks]);

    if (!featured) return null;

    return (
        <div className="rounded-lg border bg-gradient-to-br from-primary/5 to-transparent p-4">
            <div className="mb-2 flex items-center gap-1.5 text-xs font-semibold text-primary">
                <Sparkles className="size-3.5" />
                Chunk of the day
            </div>
            <Link
                to="/chunks/$chunkId"
                params={{ chunkId: featured.id }}
                className="group block"
            >
                <div className="flex items-center gap-2">
                    <h3 className="text-base font-semibold group-hover:text-primary transition-colors">
                        {featured.title}
                    </h3>
                    <Badge variant="secondary" size="sm" className="font-mono text-[9px]">
                        {featured.type}
                    </Badge>
                </div>
                {featured.summary && (
                    <p className="mt-1.5 line-clamp-2 text-xs text-muted-foreground">{featured.summary}</p>
                )}
            </Link>
        </div>
    );
}
