import {
    DndContext,
    KeyboardSensor,
    PointerSensor,
    closestCenter,
    useSensor,
    useSensors,
    type DragEndEvent,
} from "@dnd-kit/core";
import {
    SortableContext,
    arrayMove,
    sortableKeyboardCoordinates,
    useSortable,
    verticalListSortingStrategy,
} from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import { useEffect, useState, type HTMLAttributes, type ReactNode } from "react";

export interface SortableListProps<TItem> {
    /** The canonical list from the caller. The component keeps a local copy so
     *  drags feel instant; it re-syncs whenever the caller's array changes. */
    items: TItem[];
    /** Stable unique id per item (used by dnd-kit to track the drag). */
    getId: (item: TItem) => string;
    /** Called with the new order after every drop. Parent persists server-side. */
    onReorder: (ids: string[]) => void;
    /** Render one item. Spread `dragHandleProps` onto whatever element should
     *  act as the drag handle — typically a grip icon. */
    renderItem: (item: TItem, opts: { dragHandleProps: HTMLAttributes<HTMLElement> }) => ReactNode;
    /** Pixel distance before a drag activates. Prevents accidental drags on click. Default 4px. */
    activationDistance?: number;
}

/**
 * Vertical drag-to-reorder list. Two existing call-sites (requirements list
 * and plan tasks section) had ~90% identical dnd-kit wiring; this wraps them.
 *
 * Keeping the API minimal on purpose: the caller owns state + server sync.
 * Grid/horizontal strategies can land when we actually need them.
 */
export function SortableList<TItem>({
    items,
    getId,
    onReorder,
    renderItem,
    activationDistance = 4,
}: SortableListProps<TItem>) {
    const [local, setLocal] = useState(items);
    useEffect(() => setLocal(items), [items]);

    const sensors = useSensors(
        useSensor(PointerSensor, { activationConstraint: { distance: activationDistance } }),
        useSensor(KeyboardSensor, { coordinateGetter: sortableKeyboardCoordinates }),
    );

    function onDragEnd(event: DragEndEvent) {
        const { active, over } = event;
        if (!over || active.id === over.id) return;
        const oldIndex = local.findIndex(i => getId(i) === active.id);
        const newIndex = local.findIndex(i => getId(i) === over.id);
        if (oldIndex === -1 || newIndex === -1) return;
        const next = arrayMove(local, oldIndex, newIndex);
        setLocal(next);
        onReorder(next.map(getId));
    }

    const ids = local.map(getId);

    return (
        <DndContext sensors={sensors} collisionDetection={closestCenter} onDragEnd={onDragEnd}>
            <SortableContext items={ids} strategy={verticalListSortingStrategy}>
                {local.map(item => (
                    <SortableRow key={getId(item)} id={getId(item)}>
                        {dragHandleProps => renderItem(item, { dragHandleProps })}
                    </SortableRow>
                ))}
            </SortableContext>
        </DndContext>
    );
}

function SortableRow({
    id,
    children,
}: {
    id: string;
    children: (dragHandleProps: HTMLAttributes<HTMLElement>) => ReactNode;
}) {
    const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({ id });
    const style: React.CSSProperties = {
        transform: CSS.Transform.toString(transform),
        transition,
        opacity: isDragging ? 0.5 : undefined,
    };
    return (
        <div ref={setNodeRef} style={style}>
            {children({ ...attributes, ...listeners })}
        </div>
    );
}
