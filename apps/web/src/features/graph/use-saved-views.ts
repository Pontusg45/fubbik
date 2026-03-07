import { useCallback, useSyncExternalStore } from "react";

interface GraphView {
    name: string;
    filterTypes: string[];
    filterRelations: string[];
    collapsedParents: string[];
    layoutAlgorithm: string;
    focusNodeId?: string;
}

const STORAGE_KEY = "fubbik-graph-views";

// Cache the parsed result so getSnapshot returns the same reference
// unless the underlying data actually changes.
let cachedRaw: string | null = null;
let cachedViews: GraphView[] = [];

function getSnapshot(): GraphView[] {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw !== cachedRaw) {
        cachedRaw = raw;
        try {
            cachedViews = JSON.parse(raw ?? "[]");
        } catch {
            cachedViews = [];
        }
    }
    return cachedViews;
}

const serverSnapshot: GraphView[] = [];

function subscribe(cb: () => void) {
    window.addEventListener("storage", cb);
    return () => window.removeEventListener("storage", cb);
}

function persist(views: GraphView[]) {
    const json = JSON.stringify(views);
    localStorage.setItem(STORAGE_KEY, json);
    // Update cache immediately so getSnapshot returns the new value
    cachedRaw = json;
    cachedViews = views;
    window.dispatchEvent(new StorageEvent("storage", { key: STORAGE_KEY }));
}

export function useSavedGraphViews() {
    const views = useSyncExternalStore(subscribe, getSnapshot, () => serverSnapshot);

    const saveView = useCallback((view: GraphView) => {
        const current = getSnapshot();
        const next = [...current];
        const idx = next.findIndex(v => v.name === view.name);
        if (idx >= 0) next[idx] = view;
        else next.push(view);
        persist(next);
    }, []);

    const deleteView = useCallback((name: string) => {
        persist(getSnapshot().filter(v => v.name !== name));
    }, []);

    return { views, saveView, deleteView };
}
