import type { Edge } from "@xyflow/react";
import { useEffect } from "react";

import type { GraphAction } from "./use-graph-state";

export interface UseGraphKeyboardOptions {
    selectedChunkId: string | null;
    focusedNodeId: string | null;
    pathStartId: string | null;
    pathEndId: string | null;
    multiSelectedIds: Set<string>;
    layoutEdges: Edge[];
    dispatch: (action: GraphAction) => void;
}

/**
 * Handles global keyboard shortcuts for the graph view:
 * - `?` toggles help overlay
 * - `Escape` clears multi-select, path, selection, or focus (in priority order)
 * - `Tab` / `Shift+Tab` cycles through connected nodes
 */
export function useGraphKeyboard({
    selectedChunkId,
    focusedNodeId,
    pathStartId,
    pathEndId,
    multiSelectedIds,
    layoutEdges,
    dispatch,
}: UseGraphKeyboardOptions) {
    useEffect(() => {
        function handleKeyDown(e: KeyboardEvent) {
            if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return;

            if (e.key === "?") {
                dispatch({ type: "TOGGLE_HELP" });
                return;
            }

            if (e.key === "Escape") {
                if (multiSelectedIds.size > 0) {
                    dispatch({ type: "CLEAR_MULTI_SELECT" });
                    return;
                }
                if (pathStartId || pathEndId) {
                    dispatch({ type: "CLEAR_PATH" });
                    return;
                }
                if (selectedChunkId) {
                    dispatch({ type: "SET_SELECTED_CHUNK", id: null });
                } else if (focusedNodeId) {
                    dispatch({ type: "SET_FOCUSED_NODE", id: null });
                }
                return;
            }

            if (e.key === "Tab" && selectedChunkId) {
                e.preventDefault();
                const connectedIds: string[] = [];
                for (const edge of layoutEdges) {
                    if (edge.source === selectedChunkId) connectedIds.push(edge.target);
                    if (edge.target === selectedChunkId) connectedIds.push(edge.source);
                }
                if (connectedIds.length === 0) return;
                const currentIdx = connectedIds.indexOf(selectedChunkId);
                const nextIdx = e.shiftKey
                    ? (currentIdx - 1 + connectedIds.length) % connectedIds.length
                    : (currentIdx + 1) % connectedIds.length;
                const nextId = connectedIds[nextIdx]!;
                dispatch({ type: "SELECT_AND_FOCUS_NODE", id: nextId });
            }
        }

        document.addEventListener("keydown", handleKeyDown);
        return () => document.removeEventListener("keydown", handleKeyDown);
    }, [selectedChunkId, focusedNodeId, layoutEdges, pathStartId, pathEndId, multiSelectedIds, dispatch]);
}
