import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { useState } from "react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardPanel } from "@/components/ui/card";
import { Checkbox } from "@/components/ui/checkbox";
import { api } from "@/utils/api";

const TYPES = ["note", "document", "reference", "schema", "checklist"];

interface Chunk {
    id: string;
    title: string;
    type: string;
    updatedAt: string | Date;
}

interface KanbanViewProps {
    chunks: Chunk[];
    onBulkDelete?: (ids: Set<string>) => void;
    onBulkArchive?: (ids: Set<string>) => void;
}

export function KanbanView({ chunks, onBulkDelete, onBulkArchive }: KanbanViewProps) {
    const queryClient = useQueryClient();
    const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());

    const updateMutation = useMutation({
        mutationFn: async ({ id, type }: { id: string; type: string }) => {
            await api.api.chunks({ id }).patch({ type });
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["chunks-list"] });
        }
    });

    function handleDrop(e: React.DragEvent, targetType: string) {
        e.preventDefault();
        const chunkId = e.dataTransfer.getData("text/plain");
        if (chunkId) updateMutation.mutate({ id: chunkId, type: targetType });
    }

    const columns = TYPES.map(type => ({
        type,
        items: chunks.filter(c => c.type === type)
    }));

    return (
        <div className="relative">
            <div className="flex gap-3 overflow-x-auto pb-2">
                {columns.map(col => (
                    <div
                        key={col.type}
                        className="bg-muted/30 min-w-[200px] flex-1 rounded-lg border p-2"
                        onDragOver={e => e.preventDefault()}
                        onDrop={e => handleDrop(e, col.type)}
                    >
                        <div className="mb-2 flex items-center justify-between">
                            <Badge variant="secondary" size="sm">
                                {col.type}
                            </Badge>
                            <span className="text-muted-foreground text-[10px]">{col.items.length}</span>
                        </div>
                        <div className="space-y-1.5">
                            {col.items.map(chunk => (
                                <div key={chunk.id} className="flex items-start gap-1.5">
                                    <Checkbox
                                        checked={selectedIds.has(chunk.id)}
                                        onCheckedChange={() => {
                                            setSelectedIds(prev => {
                                                const next = new Set(prev);
                                                next.has(chunk.id) ? next.delete(chunk.id) : next.add(chunk.id);
                                                return next;
                                            });
                                        }}
                                        onClick={(e: React.MouseEvent) => e.stopPropagation()}
                                        className="mt-2 shrink-0"
                                    />
                                    <Link
                                        to="/chunks/$chunkId"
                                        params={{ chunkId: chunk.id }}
                                        draggable
                                        onDragStart={e => e.dataTransfer.setData("text/plain", chunk.id)}
                                        className="min-w-0 flex-1"
                                    >
                                        <Card>
                                            <CardPanel className="hover:bg-muted/50 cursor-grab p-2 text-xs transition-colors active:cursor-grabbing">
                                                <p className="truncate font-medium">{chunk.title}</p>
                                                <div className="mt-1 flex flex-wrap gap-0.5">
                                                    {([] as string[]).slice(0, 3).map(tag => (
                                                        <Badge key={tag} variant="outline" size="sm" className="text-[8px]">
                                                            {tag}
                                                        </Badge>
                                                    ))}
                                                </div>
                                            </CardPanel>
                                        </Card>
                                    </Link>
                                </div>
                            ))}
                        </div>
                    </div>
                ))}
            </div>

            {selectedIds.size > 0 && (
                <div className="bg-background fixed bottom-4 left-1/2 z-20 flex -translate-x-1/2 items-center gap-3 rounded-lg border px-4 py-2 shadow-lg">
                    <span className="text-sm font-medium">{selectedIds.size} selected</span>
                    {onBulkArchive && (
                        <Button size="sm" variant="outline" onClick={() => { onBulkArchive(selectedIds); setSelectedIds(new Set()); }}>
                            Archive
                        </Button>
                    )}
                    {onBulkDelete && (
                        <Button size="sm" variant="destructive" onClick={() => { onBulkDelete(selectedIds); setSelectedIds(new Set()); }}>
                            Delete
                        </Button>
                    )}
                    <Button size="sm" variant="ghost" onClick={() => setSelectedIds(new Set())}>
                        Cancel
                    </Button>
                </div>
            )}
        </div>
    );
}
