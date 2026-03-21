import { useQuery } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";

import { useDebouncedValue } from "@/hooks/use-debounced-value";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

interface SimilarChunksWarningProps {
    title: string;
    content: string;
    excludeId?: string;
}

export function SimilarChunksWarning({ title, content, excludeId }: SimilarChunksWarningProps) {
    const debouncedTitle = useDebouncedValue(title, 800);
    const debouncedContent = useDebouncedValue(content, 800);

    const { data: similarChunks } = useQuery({
        queryKey: ["check-similar", debouncedTitle, debouncedContent, excludeId],
        queryFn: async () => {
            const result = unwrapEden(
                await api.api.chunks["check-similar"].post({
                    title: debouncedTitle,
                    content: debouncedContent,
                    ...(excludeId ? { excludeId } : {}),
                })
            );
            return result as Array<{ id: string; title: string; type: string; similarity: number }>;
        },
        enabled: debouncedTitle.length > 3 && debouncedContent.length > 20,
        staleTime: 30_000,
    });

    if (!similarChunks?.length) return null;

    return (
        <div className="rounded-md border border-yellow-500/30 bg-yellow-500/10 px-3 py-2">
            <p className="text-xs font-medium text-yellow-600 dark:text-yellow-400">
                Semantically similar chunks found:
            </p>
            <ul className="mt-1 space-y-0.5">
                {similarChunks.map(c => (
                    <li key={c.id} className="flex items-center justify-between text-xs">
                        <Link
                            to="/chunks/$chunkId"
                            params={{ chunkId: c.id }}
                            className="text-yellow-600 underline dark:text-yellow-400"
                        >
                            {c.title}
                        </Link>
                        <span className="text-muted-foreground ml-2">
                            {Math.round(c.similarity * 100)}% match
                        </span>
                    </li>
                ))}
            </ul>
        </div>
    );
}
