import { useCallback, useReducer } from "react";

import type { LayoutAlgorithm } from "@/features/graph/layouts";

// --- State shape ---

export interface GraphState {
    // Selection
    selectedChunkId: string | null;
    multiSelectedIds: Set<string>;

    // Interaction
    pendingConnection: { source: string; target: string } | null;
    exploreMode: boolean;
    exploredNodeIds: Set<string>;
    pathStartId: string | null;
    pathEndId: string | null;
    showHelp: boolean;
    showWelcome: boolean;
    showDeleteConfirm: boolean;
    focusedNodeId: string | null;
    collapsedParents: Set<string>;
    showPathPanel: boolean;
    showSaveDialog: boolean;
    viewName: string;

    // Filter
    filterTypes: Set<string>;
    filterRelations: Set<string>;
    searchQuery: string;
    activeTagTypeIds: Set<string>;
    showUngrouped: boolean;

    // View
    layoutAlgorithm: LayoutAlgorithm;
    bundleEdges: boolean;
    useMainThread: boolean;
    timelineCutoff: Date | null;
    panelWidth: number;
    edgeAnimated: boolean;
}

// --- Actions ---

export type GraphAction =
    // Selection
    | { type: "SET_SELECTED_CHUNK"; id: string | null }
    | { type: "SET_MULTI_SELECTED"; ids: Set<string> }
    | { type: "TOGGLE_MULTI_SELECT"; id: string }
    | { type: "CLEAR_MULTI_SELECT" }

    // Interaction
    | { type: "SET_PENDING_CONNECTION"; connection: { source: string; target: string } | null }
    | { type: "SET_EXPLORE_MODE"; enabled: boolean }
    | { type: "SET_EXPLORED_NODE_IDS"; ids: Set<string> }
    | { type: "ADD_EXPLORED_NODES"; ids: string[] }
    | { type: "SET_PATH_START"; id: string | null }
    | { type: "SET_PATH_END"; id: string | null }
    | { type: "CLEAR_PATH" }
    | { type: "TOGGLE_HELP" }
    | { type: "SET_SHOW_WELCOME"; show: boolean }
    | { type: "SET_SHOW_DELETE_CONFIRM"; show: boolean }
    | { type: "SET_FOCUSED_NODE"; id: string | null }
    | { type: "TOGGLE_COLLAPSED_PARENT"; id: string }
    | { type: "SET_COLLAPSED_PARENTS"; ids: Set<string> }
    | { type: "SET_SHOW_PATH_PANEL"; show: boolean }
    | { type: "SET_SHOW_SAVE_DIALOG"; show: boolean }
    | { type: "SET_VIEW_NAME"; name: string }

    // Filter
    | { type: "TOGGLE_FILTER_TYPE"; filterType: string }
    | { type: "SET_FILTER_TYPES"; types: Set<string> }
    | { type: "TOGGLE_FILTER_RELATION"; relation: string }
    | { type: "SET_FILTER_RELATIONS"; relations: Set<string> }
    | { type: "SET_SEARCH_QUERY"; query: string }
    | { type: "TOGGLE_TAG_TYPE"; id: string }
    | { type: "SET_ACTIVE_TAG_TYPE_IDS"; ids: Set<string> }
    | { type: "TOGGLE_UNGROUPED" }

    // View
    | { type: "SET_LAYOUT_ALGORITHM"; algorithm: LayoutAlgorithm }
    | { type: "TOGGLE_BUNDLE_EDGES" }
    | { type: "TOGGLE_USE_MAIN_THREAD" }
    | { type: "SET_TIMELINE_CUTOFF"; cutoff: Date | null }
    | { type: "SET_PANEL_WIDTH"; width: number }
    | { type: "TOGGLE_EDGE_ANIMATED" }

    // Compound
    | { type: "SELECT_AND_FOCUS_NODE"; id: string }
    | { type: "DESELECT_ALL" }
    | { type: "RESTORE_VIEW"; filterTypes: string[]; filterRelations: string[]; collapsedParents: string[]; layoutAlgorithm: LayoutAlgorithm; focusNodeId?: string };

// --- Initial state ---

export const initialGraphState: GraphState = {
    selectedChunkId: null,
    multiSelectedIds: new Set(),
    pendingConnection: null,
    exploreMode: false,
    exploredNodeIds: new Set(),
    pathStartId: null,
    pathEndId: null,
    showHelp: false,
    showWelcome: false,
    showDeleteConfirm: false,
    focusedNodeId: null,
    collapsedParents: new Set(),
    showPathPanel: false,
    showSaveDialog: false,
    viewName: "",
    filterTypes: new Set(),
    filterRelations: new Set(),
    searchQuery: "",
    activeTagTypeIds: new Set(),
    showUngrouped: true,
    layoutAlgorithm: "force",
    bundleEdges: false,
    useMainThread: false,
    timelineCutoff: null,
    panelWidth: 380,
    edgeAnimated: true,
};

// --- Reducer ---

function toggleSetItem<T>(set: Set<T>, item: T): Set<T> {
    const next = new Set(set);
    if (next.has(item)) next.delete(item);
    else next.add(item);
    return next;
}

export function graphReducer(state: GraphState, action: GraphAction): GraphState {
    switch (action.type) {
        // Selection
        case "SET_SELECTED_CHUNK":
            return { ...state, selectedChunkId: action.id };
        case "SET_MULTI_SELECTED":
            return { ...state, multiSelectedIds: action.ids };
        case "TOGGLE_MULTI_SELECT":
            return { ...state, multiSelectedIds: toggleSetItem(state.multiSelectedIds, action.id) };
        case "CLEAR_MULTI_SELECT":
            return { ...state, multiSelectedIds: new Set() };

        // Interaction
        case "SET_PENDING_CONNECTION":
            return { ...state, pendingConnection: action.connection };
        case "SET_EXPLORE_MODE":
            return {
                ...state,
                exploreMode: action.enabled,
                exploredNodeIds: action.enabled ? state.exploredNodeIds : new Set(),
            };
        case "SET_EXPLORED_NODE_IDS":
            return { ...state, exploredNodeIds: action.ids };
        case "ADD_EXPLORED_NODES": {
            const next = new Set(state.exploredNodeIds);
            for (const id of action.ids) next.add(id);
            return { ...state, exploredNodeIds: next };
        }
        case "SET_PATH_START":
            return { ...state, pathStartId: action.id };
        case "SET_PATH_END":
            return { ...state, pathEndId: action.id };
        case "CLEAR_PATH":
            return { ...state, pathStartId: null, pathEndId: null };
        case "TOGGLE_HELP":
            return { ...state, showHelp: !state.showHelp };
        case "SET_SHOW_WELCOME":
            return { ...state, showWelcome: action.show };
        case "SET_SHOW_DELETE_CONFIRM":
            return { ...state, showDeleteConfirm: action.show };
        case "SET_FOCUSED_NODE":
            return { ...state, focusedNodeId: action.id };
        case "TOGGLE_COLLAPSED_PARENT":
            return { ...state, collapsedParents: toggleSetItem(state.collapsedParents, action.id) };
        case "SET_COLLAPSED_PARENTS":
            return { ...state, collapsedParents: action.ids };
        case "SET_SHOW_PATH_PANEL":
            return { ...state, showPathPanel: action.show };
        case "SET_SHOW_SAVE_DIALOG":
            return { ...state, showSaveDialog: action.show };
        case "SET_VIEW_NAME":
            return { ...state, viewName: action.name };

        // Filter
        case "TOGGLE_FILTER_TYPE":
            return { ...state, filterTypes: toggleSetItem(state.filterTypes, action.filterType) };
        case "SET_FILTER_TYPES":
            return { ...state, filterTypes: action.types };
        case "TOGGLE_FILTER_RELATION":
            return { ...state, filterRelations: toggleSetItem(state.filterRelations, action.relation) };
        case "SET_FILTER_RELATIONS":
            return { ...state, filterRelations: action.relations };
        case "SET_SEARCH_QUERY":
            return { ...state, searchQuery: action.query };
        case "TOGGLE_TAG_TYPE": {
            const next = state.activeTagTypeIds.has(action.id) ? new Set<string>() : new Set([action.id]);
            return { ...state, activeTagTypeIds: next };
        }
        case "SET_ACTIVE_TAG_TYPE_IDS":
            return { ...state, activeTagTypeIds: action.ids };
        case "TOGGLE_UNGROUPED":
            return { ...state, showUngrouped: !state.showUngrouped };

        // View
        case "SET_LAYOUT_ALGORITHM":
            return { ...state, layoutAlgorithm: action.algorithm };
        case "TOGGLE_BUNDLE_EDGES":
            return { ...state, bundleEdges: !state.bundleEdges };
        case "TOGGLE_USE_MAIN_THREAD":
            return { ...state, useMainThread: !state.useMainThread };
        case "SET_TIMELINE_CUTOFF":
            return { ...state, timelineCutoff: action.cutoff };
        case "SET_PANEL_WIDTH":
            return { ...state, panelWidth: action.width };
        case "TOGGLE_EDGE_ANIMATED":
            return { ...state, edgeAnimated: !state.edgeAnimated };

        // Compound
        case "SELECT_AND_FOCUS_NODE":
            return { ...state, selectedChunkId: action.id, focusedNodeId: action.id };
        case "DESELECT_ALL":
            return { ...state, selectedChunkId: null, focusedNodeId: null };
        case "RESTORE_VIEW":
            return {
                ...state,
                filterTypes: new Set(action.filterTypes),
                filterRelations: new Set(action.filterRelations),
                collapsedParents: new Set(action.collapsedParents),
                layoutAlgorithm: action.layoutAlgorithm,
                ...(action.focusNodeId
                    ? { selectedChunkId: action.focusNodeId, focusedNodeId: action.focusNodeId }
                    : {}),
            };

        default:
            return state;
    }
}

// --- Hook ---

export function useGraphState() {
    const [state, dispatch] = useReducer(graphReducer, initialGraphState);

    const handleTimelineCutoff = useCallback((d: Date | null) => {
        dispatch({ type: "SET_TIMELINE_CUTOFF", cutoff: d });
    }, []);

    return { state, dispatch, handleTimelineCutoff };
}
