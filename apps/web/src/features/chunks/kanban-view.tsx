import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";

import { Badge } from "@/components/ui/badge";
import { Card, CardPanel } from "@/components/ui/card";
import { api } from "@/utils/api";

const TYPES = ["note", "document", "reference", "schema", "checklist"];

interface Chunk {
    id: string;
    title: string;
    type: string;
    updatedAt: string | Date;
}

export function KanbanView({ chunks }: { chunks: Chunk[] }) {
    const queryClient = useQueryClient();

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
                            <Link
                                key={chunk.id}
                                to="/chunks/$chunkId"
                                params={{ chunkId: chunk.id }}
                                draggable
                                onDragStart={e => e.dataTransfer.setData("text/plain", chunk.id)}
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
                        ))}
                    </div>
                </div>
            ))}
        </div>
    );
}
