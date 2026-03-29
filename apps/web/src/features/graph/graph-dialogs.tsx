import type { UseMutationResult } from "@tanstack/react-query";
import { Dialog, DialogPopup, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { relationColor } from "@/features/chunks/relation-colors";
import type { GraphAction } from "./use-graph-state";
import type { LayoutAlgorithm } from "./layouts";

// ---------------------------------------------------------------------------
// Shared constants
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// ChangeConnectionDialog
// ---------------------------------------------------------------------------

interface ChangeConnectionDialogProps {
    pendingConnection: { source: string; target: string } | null;
    chunkMap: Map<string, { title: string }>;
    createConnectionMutation: UseMutationResult<void, Error, { sourceId: string; targetId: string; relation: string }>;
    dispatch: React.Dispatch<GraphAction>;
}

export function ChangeConnectionDialog({
    pendingConnection,
    chunkMap,
    createConnectionMutation,
    dispatch,
}: ChangeConnectionDialogProps) {
    return (
        <Dialog
            open={!!pendingConnection}
            onOpenChange={open => {
                if (!open) dispatch({ type: "SET_PENDING_CONNECTION", connection: null });
            }}
        >
            <DialogPopup className="max-w-sm">
                <DialogHeader>
                    <DialogTitle>Change Connection Type</DialogTitle>
                    <p className="text-muted-foreground text-sm">
                        <span className="text-foreground font-medium">
                            {pendingConnection ? (chunkMap.get(pendingConnection.source)?.title ?? pendingConnection.source) : ""}
                        </span>
                        {" \u2192 "}
                        <span className="text-foreground font-medium">
                            {pendingConnection ? (chunkMap.get(pendingConnection.target)?.title ?? pendingConnection.target) : ""}
                        </span>
                    </p>
                </DialogHeader>
                <div className="grid grid-cols-2 gap-2 px-6 pb-6">
                    {RELATION_TYPES.map(rel => (
                        <button
                            key={rel}
                            disabled={createConnectionMutation.isPending}
                            onClick={() => {
                                if (!pendingConnection) return;
                                createConnectionMutation.mutate({
                                    sourceId: pendingConnection.source,
                                    targetId: pendingConnection.target,
                                    relation: rel
                                });
                            }}
                            className="hover:bg-muted rounded-md border-2 px-3 py-2 text-sm font-medium transition-colors disabled:opacity-50"
                            style={{ borderColor: relationColor(rel) }}
                        >
                            {rel.replace(/_/g, " ")}
                        </button>
                    ))}
                </div>
            </DialogPopup>
        </Dialog>
    );
}

// ---------------------------------------------------------------------------
// SaveViewDialog
// ---------------------------------------------------------------------------

interface SaveViewDialogProps {
    show: boolean;
    viewName: string;
    filterTypes: Set<string>;
    filterRelations: Set<string>;
    collapsedParents: Set<string>;
    layoutAlgorithm: LayoutAlgorithm;
    focusedNodeId: string | null;
    saveView: (view: {
        name: string;
        filterTypes: string[];
        filterRelations: string[];
        collapsedParents: string[];
        layoutAlgorithm: string;
        focusNodeId?: string;
    }) => void;
    dispatch: React.Dispatch<GraphAction>;
}

export function SaveViewDialog({
    show,
    viewName,
    filterTypes,
    filterRelations,
    collapsedParents,
    layoutAlgorithm,
    focusedNodeId,
    saveView,
    dispatch,
}: SaveViewDialogProps) {
    if (!show) return null;

    const handleSave = () => {
        if (!viewName.trim()) return;
        saveView({
            name: viewName.trim(),
            filterTypes: [...filterTypes],
            filterRelations: [...filterRelations],
            collapsedParents: [...collapsedParents],
            layoutAlgorithm,
            focusNodeId: focusedNodeId ?? undefined
        });
        dispatch({ type: "SET_SHOW_SAVE_DIALOG", show: false });
        dispatch({ type: "SET_VIEW_NAME", name: "" });
    };

    return (
        <div
            className="bg-background/50 absolute inset-0 z-30 flex items-center justify-center backdrop-blur-sm"
            onClick={() => dispatch({ type: "SET_SHOW_SAVE_DIALOG", show: false })}
        >
            <div className="bg-background rounded-lg border p-4 shadow-lg" onClick={e => e.stopPropagation()}>
                <h3 className="mb-2 text-sm font-semibold">Save View</h3>
                <input
                    value={viewName}
                    onChange={e => dispatch({ type: "SET_VIEW_NAME", name: e.target.value })}
                    placeholder="View name"
                    className="w-full rounded-md border px-3 py-2 text-sm"
                    autoFocus
                    onKeyDown={e => {
                        if (e.key === "Enter" && viewName.trim()) {
                            handleSave();
                        }
                    }}
                />
                <div className="mt-3 flex gap-2">
                    <button
                        onClick={handleSave}
                        className="bg-primary text-primary-foreground flex-1 rounded-md px-3 py-1.5 text-xs"
                    >
                        Save
                    </button>
                    <button onClick={() => dispatch({ type: "SET_SHOW_SAVE_DIALOG", show: false })} className="rounded-md border px-3 py-1.5 text-xs">
                        Cancel
                    </button>
                </div>
            </div>
        </div>
    );
}

// ---------------------------------------------------------------------------
// SaveCustomGraphDialog
// ---------------------------------------------------------------------------

interface SaveCustomGraphDialogProps {
    show: boolean;
    onClose: () => void;
    customGraphName: string;
    onNameChange: (name: string) => void;
    visibleChunkCount: number;
    filteredChunkIds: string[];
    draggedPositions: Map<string, { x: number; y: number }>;
    layoutPositions: Record<string, { x: number; y: number }> | null;
    layoutAlgorithm: LayoutAlgorithm;
    codebaseId: string | null | undefined;
    saveCustomGraphMutation: UseMutationResult<unknown, Error, {
        name: string;
        chunkIds: string[];
        positions: Record<string, { x: number; y: number }>;
        layoutAlgorithm: string;
        codebaseId?: string | null;
    }>;
}

export function SaveCustomGraphDialog({
    show,
    onClose,
    customGraphName,
    onNameChange,
    visibleChunkCount,
    filteredChunkIds,
    draggedPositions,
    layoutPositions,
    layoutAlgorithm,
    codebaseId,
    saveCustomGraphMutation,
}: SaveCustomGraphDialogProps) {
    if (!show) return null;

    const handleSave = () => {
        if (!customGraphName.trim() || filteredChunkIds.length === 0) return;
        const positions: Record<string, { x: number; y: number }> = {};
        for (const id of filteredChunkIds) {
            const dragged = draggedPositions.get(id);
            const lp = layoutPositions?.[id];
            if (dragged) positions[id] = dragged;
            else if (lp) positions[id] = lp;
        }
        saveCustomGraphMutation.mutate({
            name: customGraphName.trim(),
            chunkIds: filteredChunkIds,
            positions,
            layoutAlgorithm,
            codebaseId: codebaseId && codebaseId !== "global" ? codebaseId : undefined
        });
        onClose();
        onNameChange("");
    };

    return (
        <div
            className="bg-background/50 absolute inset-0 z-30 flex items-center justify-center backdrop-blur-sm"
            onClick={onClose}
        >
            <div className="bg-background w-80 rounded-lg border p-4 shadow-lg" onClick={e => e.stopPropagation()}>
                <h3 className="mb-2 text-sm font-semibold">Save as Custom Graph</h3>
                <p className="text-muted-foreground mb-3 text-xs">
                    Saves {visibleChunkCount} visible chunks with their current positions.
                </p>
                <input
                    value={customGraphName}
                    onChange={e => onNameChange(e.target.value)}
                    placeholder="Graph name"
                    className="w-full rounded-md border px-3 py-2 text-sm"
                    autoFocus
                    onKeyDown={e => {
                        if (e.key === "Enter" && customGraphName.trim()) {
                            handleSave();
                        }
                    }}
                />
                <div className="mt-3 flex gap-2">
                    <button
                        onClick={handleSave}
                        disabled={saveCustomGraphMutation.isPending}
                        className="bg-primary text-primary-foreground flex-1 rounded-md px-3 py-1.5 text-xs"
                    >
                        {saveCustomGraphMutation.isPending ? "Saving..." : "Save"}
                    </button>
                    <button onClick={onClose} className="rounded-md border px-3 py-1.5 text-xs">
                        Cancel
                    </button>
                </div>
            </div>
        </div>
    );
}
