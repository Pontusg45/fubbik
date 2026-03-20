import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ChevronDown, ChevronUp, Link2, Search } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Dialog, DialogHeader, DialogPopup, DialogTitle, DialogTrigger } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { api } from "@/utils/api";

const RELATION_TYPES = [
    "related_to",
    "part_of",
    "depends_on",
    "extends",
    "references",
    "supports",
    "contradicts",
    "alternative_to"
] as const;

function formatRelation(r: string) {
    return r.replace(/_/g, " ");
}

export function LinkChunkDialog({ chunkId }: { chunkId: string }) {
    const [open, setOpen] = useState(false);
    const [search, setSearch] = useState("");
    const [relation, setRelation] = useState("related_to");
    const [showRelationPicker, setShowRelationPicker] = useState(false);
    const queryClient = useQueryClient();

    const { data: searchResults } = useQuery({
        queryKey: ["chunks", "search", search],
        queryFn: async () => {
            if (!search.trim()) return { chunks: [] };
            const { data, error } = await api.api.chunks.get({ query: { search, limit: "10" } });
            if (error) throw new Error("Failed to search chunks");
            return data;
        },
        enabled: open && search.trim().length > 0
    });

    const createMutation = useMutation({
        mutationFn: async (targetId: string) => {
            const { error } = await api.api.connections.post({
                sourceId: chunkId,
                targetId,
                relation
            });
            if (error) throw new Error("Failed to create connection");
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["chunk", chunkId] });
            toast.success("Connection created");
            setSearch("");
            setShowRelationPicker(false);
            setOpen(false);
        },
        onError: () => {
            toast.error("Failed to create connection");
        }
    });

    const filteredResults = (searchResults?.chunks ?? []).filter(c => c.id !== chunkId);

    return (
        <Dialog open={open} onOpenChange={setOpen}>
            <DialogTrigger render={<Button variant="outline" size="sm" />}>
                <Link2 className="size-3.5" />
                Link Chunk
            </DialogTrigger>
            <DialogPopup>
                <DialogHeader>
                    <DialogTitle>Link Chunk</DialogTitle>
                </DialogHeader>
                <div className="space-y-4 px-6 pb-6">
                    <div className="space-y-2">
                        <label className="text-muted-foreground text-xs font-medium">Search chunks</label>
                        <div className="relative">
                            <Search className="text-muted-foreground pointer-events-none absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2" />
                            <Input
                                value={search}
                                onChange={e => setSearch((e.target as HTMLInputElement).value)}
                                placeholder="Type to search..."
                                className="pl-8"
                            />
                        </div>
                    </div>
                    <div className="flex items-center gap-2">
                        <Badge variant="secondary" size="sm">
                            {formatRelation(relation)}
                        </Badge>
                        <button
                            type="button"
                            onClick={() => setShowRelationPicker(v => !v)}
                            className="text-muted-foreground hover:text-foreground flex items-center gap-0.5 text-xs transition-colors"
                        >
                            Change type
                            {showRelationPicker ? (
                                <ChevronUp className="size-3" />
                            ) : (
                                <ChevronDown className="size-3" />
                            )}
                        </button>
                    </div>
                    {showRelationPicker && (
                        <div className="flex flex-wrap gap-1.5">
                            {RELATION_TYPES.map(r => (
                                <button
                                    key={r}
                                    type="button"
                                    onClick={() => {
                                        setRelation(r);
                                        setShowRelationPicker(false);
                                    }}
                                    className={`rounded-md border px-2 py-1 text-xs transition-colors ${
                                        relation === r
                                            ? "bg-primary text-primary-foreground border-transparent"
                                            : "hover:bg-muted border-border"
                                    }`}
                                >
                                    {formatRelation(r)}
                                </button>
                            ))}
                        </div>
                    )}
                    {filteredResults.length > 0 && (
                        <div className="max-h-60 space-y-1 overflow-y-auto rounded-md border p-1">
                            {filteredResults.map(result => (
                                <button
                                    key={result.id}
                                    type="button"
                                    onClick={() => createMutation.mutate(result.id)}
                                    disabled={createMutation.isPending}
                                    className="hover:bg-muted flex w-full items-center justify-between rounded-md px-3 py-2 text-left text-sm transition-colors"
                                >
                                    <span className="font-medium">{result.title}</span>
                                    <Badge variant="secondary" size="sm" className="text-[10px]">
                                        {result.type}
                                    </Badge>
                                </button>
                            ))}
                        </div>
                    )}
                    {search.trim().length > 0 && filteredResults.length === 0 && (
                        <p className="text-muted-foreground py-4 text-center text-sm">No chunks found</p>
                    )}
                </div>
            </DialogPopup>
        </Dialog>
    );
}
