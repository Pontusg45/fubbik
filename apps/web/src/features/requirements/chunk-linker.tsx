import { useQuery } from "@tanstack/react-query";
import { useState } from "react";

import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

interface ChunkLinkerProps {
    selectedChunkIds: string[];
    onSelectedChunkIdsChange: (ids: string[]) => void;
    codebaseId: string | undefined | null;
}

export function ChunkLinker({ selectedChunkIds, onSelectedChunkIdsChange, codebaseId }: ChunkLinkerProps) {
    const [chunkSearch, setChunkSearch] = useState("");

    const chunksQuery = useQuery({
        queryKey: ["chunks-for-linking", codebaseId],
        queryFn: async () => {
            try {
                const query: { codebaseId?: string; limit?: string } = { limit: "100" };
                if (codebaseId) query.codebaseId = codebaseId;
                const result = unwrapEden(await api.api.chunks.get({ query })) as {
                    chunks?: Array<{ id: string; title: string }>;
                } | null;
                return result?.chunks ?? [];
            } catch {
                return [];
            }
        }
    });

    const allChunks = chunksQuery.data ?? [];
    const filteredChunks = chunkSearch
        ? allChunks.filter(
              c => c.title.toLowerCase().includes(chunkSearch.toLowerCase()) && !selectedChunkIds.includes(c.id)
          )
        : allChunks.filter(c => !selectedChunkIds.includes(c.id));

    return (
        <div>
            <label className="mb-1.5 block text-sm font-medium">Linked Chunks (optional)</label>

            {selectedChunkIds.length > 0 && (
                <div className="mb-2 flex flex-wrap gap-2">
                    {selectedChunkIds.map(id => {
                        const c = allChunks.find(ch => ch.id === id);
                        return (
                            <Badge
                                key={id}
                                variant="secondary"
                                size="sm"
                                className="cursor-pointer"
                                onClick={() => onSelectedChunkIdsChange(selectedChunkIds.filter(cid => cid !== id))}
                            >
                                {c?.title ?? id.slice(0, 8)} x
                            </Badge>
                        );
                    })}
                </div>
            )}

            <Input
                value={chunkSearch}
                onChange={e => setChunkSearch(e.target.value)}
                placeholder="Search chunks to link..."
            />

            {chunkSearch && filteredChunks.length > 0 && (
                <div className="mt-1 max-h-40 overflow-y-auto rounded-md border">
                    {filteredChunks.slice(0, 10).map(c => (
                        <button
                            key={c.id}
                            type="button"
                            onClick={() => {
                                onSelectedChunkIdsChange([...selectedChunkIds, c.id]);
                                setChunkSearch("");
                            }}
                            className="hover:bg-muted w-full px-3 py-1.5 text-left text-sm transition-colors"
                        >
                            {c.title}
                        </button>
                    ))}
                </div>
            )}
        </div>
    );
}
