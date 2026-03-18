import {
    DndContext,
    closestCenter,
    KeyboardSensor,
    PointerSensor,
    useSensor,
    useSensors,
    type DragEndEvent
} from "@dnd-kit/core";
import {
    SortableContext,
    sortableKeyboardCoordinates,
    useSortable,
    verticalListSortingStrategy,
    arrayMove
} from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { toast } from "sonner";

import { RequirementCard } from "./requirement-card";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

interface SortableRequirementListProps {
    requirements: Array<Record<string, unknown>>;
    selectedIds: string[];
    onToggleSelection: (id: string, selected: boolean) => void;
    useCaseMap: Map<string, { name: string }>;
}

function SortableItem({
    id,
    children
}: {
    id: string;
    children: (props: {
        dragHandleProps: React.HTMLAttributes<HTMLDivElement>;
        style: React.CSSProperties;
    }) => React.ReactNode;
}) {
    const { attributes, listeners, setNodeRef, transform, transition } =
        useSortable({ id });
    const style: React.CSSProperties = {
        transform: CSS.Transform.toString(transform),
        transition
    };
    return (
        <div ref={setNodeRef} style={style}>
            {children({ dragHandleProps: { ...attributes, ...listeners }, style })}
        </div>
    );
}

export function SortableRequirementList({
    requirements,
    selectedIds,
    onToggleSelection,
    useCaseMap
}: SortableRequirementListProps) {
    const queryClient = useQueryClient();
    const [localOrder, setLocalOrder] = useState(requirements);

    // Sync local order when requirements change from server
    useEffect(() => {
        setLocalOrder(requirements);
    }, [requirements]);

    const sensors = useSensors(
        useSensor(PointerSensor),
        useSensor(KeyboardSensor, {
            coordinateGetter: sortableKeyboardCoordinates
        })
    );

    const reorderMutation = useMutation({
        mutationFn: async (requirementIds: string[]) => {
            return unwrapEden(
                await api.api.requirements.reorder.patch({
                    requirementIds
                })
            );
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["requirements"] });
        },
        onError: () => {
            // Revert to server order
            setLocalOrder(requirements);
            toast.error("Failed to reorder requirements");
        }
    });

    function handleDragEnd(event: DragEndEvent) {
        const { active, over } = event;
        if (!over || active.id === over.id) return;

        const oldIndex = localOrder.findIndex(r => (r.id as string) === active.id);
        const newIndex = localOrder.findIndex(r => (r.id as string) === over.id);

        if (oldIndex === -1 || newIndex === -1) return;

        const newOrder = arrayMove(localOrder, oldIndex, newIndex);
        setLocalOrder(newOrder);
        reorderMutation.mutate(newOrder.map(r => r.id as string));
    }

    const ids = localOrder.map(r => r.id as string);

    return (
        <DndContext
            sensors={sensors}
            collisionDetection={closestCenter}
            onDragEnd={handleDragEnd}
        >
            <SortableContext items={ids} strategy={verticalListSortingStrategy}>
                {localOrder.map(req => {
                    const id = req.id as string;
                    const useCaseId = req.useCaseId as string | null | undefined;
                    const uc = useCaseId ? useCaseMap.get(useCaseId) : undefined;

                    return (
                        <SortableItem key={id} id={id}>
                            {({ dragHandleProps }) => (
                                <RequirementCard
                                    id={id}
                                    title={req.title as string}
                                    status={(req.status as string) ?? "untested"}
                                    priority={
                                        (req.priority as string | null) ?? null
                                    }
                                    steps={
                                        (req.steps as Array<{
                                            keyword: string;
                                            text: string;
                                        }>) ?? []
                                    }
                                    origin={(req.origin as string) ?? "human"}
                                    reviewStatus={
                                        (req.reviewStatus as string) ?? "draft"
                                    }
                                    useCaseName={uc?.name}
                                    chunkCount={
                                        req.chunkCount as number | undefined
                                    }
                                    selected={selectedIds.includes(id)}
                                    onSelectChange={selected =>
                                        onToggleSelection(id, selected)
                                    }
                                    dragHandleProps={dragHandleProps}
                                />
                            )}
                        </SortableItem>
                    );
                })}
            </SortableContext>
        </DndContext>
    );
}
