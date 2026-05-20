import { useReducer } from "react";

// --- State shape ---

export interface GraphState {
    selectedChunkId: string | null;
    multiSelectedIds: Set<string>;
    pathStartId: string | null;
    pathEndId: string | null;
    showHelp: boolean;
    showPathPanel: boolean;
    showSaveDialog: boolean;
    showDeleteConfirm: boolean;
    viewName: string;
    filterTypes: Set<string>;
    filterRelations: Set<string>;
    searchQuery: string;
    groupingTagTypeId: string | null;
    showUngrouped: boolean;
    panelWidth: number;
    heatmapMode: boolean;
}

// --- Actions ---

export type GraphAction =
    // Selection
    | { type: "SET_SELECTED_CHUNK"; id: string | null }
    | { type: "SET_MULTI_SELECTED"; ids: Set<string> }
    | { type: "TOGGLE_MULTI_SELECT"; id: string }
    | { type: "CLEAR_MULTI_SELECT" }

    // Path
    | { type: "SET_PATH_START"; id: string | null }
    | { type: "SET_PATH_END"; id: string | null }
    | { type: "CLEAR_PATH" }

    // UI toggles
    | { type: "TOGGLE_HELP" }
    | { type: "SET_SHOW_PATH_PANEL"; show: boolean }
    | { type: "SET_SHOW_SAVE_DIALOG"; show: boolean }
    | { type: "SET_SHOW_DELETE_CONFIRM"; show: boolean }
    | { type: "SET_VIEW_NAME"; name: string }

    // Filter
    | { type: "TOGGLE_FILTER_TYPE"; filterType: string }
    | { type: "SET_FILTER_TYPES"; types: Set<string> }
    | { type: "TOGGLE_FILTER_RELATION"; relation: string }
    | { type: "SET_FILTER_RELATIONS"; relations: Set<string> }
    | { type: "SET_SEARCH_QUERY"; query: string }

    // Grouping
    | { type: "SET_GROUPING_TAG_TYPE"; id: string | null }
    | { type: "TOGGLE_UNGROUPED" }

    // View
    | { type: "SET_PANEL_WIDTH"; width: number }
    | { type: "TOGGLE_HEATMAP" }

    // Compound
    | { type: "DESELECT_ALL" }
    | { type: "RESTORE_VIEW"; filterTypes: string[]; filterRelations: string[]; focusNodeId?: string };

// --- Initial state ---

export const initialGraphState: GraphState = {
    selectedChunkId: null,
    multiSelectedIds: new Set(),
    pathStartId: null,
    pathEndId: null,
    showHelp: false,
    showPathPanel: false,
    showSaveDialog: false,
    showDeleteConfirm: false,
    viewName: "",
    filterTypes: new Set(),
    filterRelations: new Set(),
    searchQuery: "",
    groupingTagTypeId: null,
    showUngrouped: false,
    panelWidth: 380,
    heatmapMode: false,
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

        // Path
        case "SET_PATH_START":
            return { ...state, pathStartId: action.id };
        case "SET_PATH_END":
            return { ...state, pathEndId: action.id };
        case "CLEAR_PATH":
            return { ...state, pathStartId: null, pathEndId: null };

        // UI toggles
        case "TOGGLE_HELP":
            return { ...state, showHelp: !state.showHelp };
        case "SET_SHOW_PATH_PANEL":
            return { ...state, showPathPanel: action.show };
        case "SET_SHOW_SAVE_DIALOG":
            return { ...state, showSaveDialog: action.show };
        case "SET_SHOW_DELETE_CONFIRM":
            return { ...state, showDeleteConfirm: action.show };
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

        // Grouping
        case "SET_GROUPING_TAG_TYPE":
            return { ...state, groupingTagTypeId: action.id };
        case "TOGGLE_UNGROUPED":
            return { ...state, showUngrouped: !state.showUngrouped };

        // View
        case "SET_PANEL_WIDTH":
            return { ...state, panelWidth: action.width };
        case "TOGGLE_HEATMAP":
            return { ...state, heatmapMode: !state.heatmapMode };

        // Compound
        case "DESELECT_ALL":
            return { ...state, selectedChunkId: null };
        case "RESTORE_VIEW":
            return {
                ...state,
                filterTypes: new Set(action.filterTypes),
                filterRelations: new Set(action.filterRelations),
                ...(action.focusNodeId
                    ? { selectedChunkId: action.focusNodeId }
                    : {}),
            };

        default:
            return state;
    }
}

// --- Hook ---

export function useGraphState() {
    const [state, dispatch] = useReducer(graphReducer, initialGraphState);

    return { state, dispatch };
}
