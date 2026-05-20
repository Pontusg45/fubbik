import { useCallback, useMemo, useReducer } from "react";

export type ZoomLevel = "overview" | "neighborhood" | "detail";

export interface ZoomState {
    level: ZoomLevel;
    focusChunkId: string | null;
    activeIslandId: string | null;
    detailChunkId: string | null;
    breadcrumbs: Array<{ level: ZoomLevel; label: string; islandId?: string; chunkId?: string }>;
}

type ZoomAction =
    | { type: "ZOOM_TO_ISLAND"; islandId: string; focusChunkId: string }
    | { type: "ZOOM_TO_CHUNK"; chunkId: string; islandId: string }
    | { type: "OPEN_DETAIL"; chunkId: string }
    | { type: "CLOSE_DETAIL" }
    | { type: "ZOOM_TO_OVERVIEW" }
    | { type: "GO_BACK" }
    | { type: "SET_FOCUS_CHUNK"; chunkId: string };

const INITIAL_STATE: ZoomState = {
    level: "overview",
    focusChunkId: null,
    activeIslandId: null,
    detailChunkId: null,
    breadcrumbs: [{ level: "overview", label: "Overview" }],
};

function zoomReducer(state: ZoomState, action: ZoomAction): ZoomState {
    switch (action.type) {
        case "ZOOM_TO_ISLAND":
            return {
                level: "neighborhood",
                focusChunkId: action.focusChunkId,
                activeIslandId: action.islandId,
                detailChunkId: null,
                breadcrumbs: [
                    { level: "overview", label: "Overview" },
                    { level: "neighborhood", label: action.islandId, islandId: action.islandId, chunkId: action.focusChunkId },
                ],
            };
        case "ZOOM_TO_CHUNK":
            return {
                level: "neighborhood",
                focusChunkId: action.chunkId,
                activeIslandId: action.islandId,
                detailChunkId: null,
                breadcrumbs: [
                    { level: "overview", label: "Overview" },
                    { level: "neighborhood", label: action.islandId, islandId: action.islandId, chunkId: action.chunkId },
                ],
            };
        case "SET_FOCUS_CHUNK":
            return { ...state, focusChunkId: action.chunkId };
        case "OPEN_DETAIL":
            return {
                ...state,
                level: "detail",
                detailChunkId: action.chunkId,
                breadcrumbs: [
                    ...state.breadcrumbs.filter(b => b.level !== "detail"),
                    { level: "detail", label: "Detail", chunkId: action.chunkId },
                ],
            };
        case "CLOSE_DETAIL":
            return {
                ...state,
                level: "neighborhood",
                detailChunkId: null,
                breadcrumbs: state.breadcrumbs.filter(b => b.level !== "detail"),
            };
        case "ZOOM_TO_OVERVIEW":
            return INITIAL_STATE;
        case "GO_BACK": {
            if (state.level === "detail") return zoomReducer(state, { type: "CLOSE_DETAIL" });
            if (state.level === "neighborhood") return INITIAL_STATE;
            return state;
        }
        default:
            return state;
    }
}

export function useGraphZoom(initialFocusChunkId?: string, initialIslandId?: string) {
    const initialState = useMemo<ZoomState>(() => {
        if (initialFocusChunkId && initialIslandId) {
            return {
                level: "neighborhood",
                focusChunkId: initialFocusChunkId,
                activeIslandId: initialIslandId,
                detailChunkId: null,
                breadcrumbs: [
                    { level: "overview", label: "Overview" },
                    { level: "neighborhood", label: initialIslandId, islandId: initialIslandId, chunkId: initialFocusChunkId },
                ],
            };
        }
        return INITIAL_STATE;
    }, [initialFocusChunkId, initialIslandId]);

    const [zoom, dispatchZoom] = useReducer(zoomReducer, initialState);

    const zoomToIsland = useCallback((islandId: string, focusChunkId: string) => {
        dispatchZoom({ type: "ZOOM_TO_ISLAND", islandId, focusChunkId });
    }, []);
    const zoomToChunk = useCallback((chunkId: string, islandId: string) => {
        dispatchZoom({ type: "ZOOM_TO_CHUNK", chunkId, islandId });
    }, []);
    const openDetail = useCallback((chunkId: string) => {
        dispatchZoom({ type: "OPEN_DETAIL", chunkId });
    }, []);
    const closeDetail = useCallback(() => { dispatchZoom({ type: "CLOSE_DETAIL" }); }, []);
    const goBack = useCallback(() => { dispatchZoom({ type: "GO_BACK" }); }, []);
    const goToOverview = useCallback(() => { dispatchZoom({ type: "ZOOM_TO_OVERVIEW" }); }, []);
    const setFocusChunk = useCallback((chunkId: string) => {
        dispatchZoom({ type: "SET_FOCUS_CHUNK", chunkId });
    }, []);

    return { zoom, zoomToIsland, zoomToChunk, openDetail, closeDetail, goBack, goToOverview, setFocusChunk };
}
