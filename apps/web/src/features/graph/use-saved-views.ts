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

function getSnapshot(): GraphView[] {
	try {
		return JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "[]");
	} catch {
		return [];
	}
}

function subscribe(cb: () => void) {
	window.addEventListener("storage", cb);
	return () => window.removeEventListener("storage", cb);
}

function persist(views: GraphView[]) {
	localStorage.setItem(STORAGE_KEY, JSON.stringify(views));
	window.dispatchEvent(new StorageEvent("storage", { key: STORAGE_KEY }));
}

export function useSavedGraphViews() {
	const views = useSyncExternalStore(subscribe, getSnapshot, () => []);

	const saveView = useCallback((view: GraphView) => {
		const current = getSnapshot();
		const idx = current.findIndex((v) => v.name === view.name);
		if (idx >= 0) current[idx] = view;
		else current.push(view);
		persist(current);
	}, []);

	const deleteView = useCallback((name: string) => {
		persist(getSnapshot().filter((v) => v.name !== name));
	}, []);

	return { views, saveView, deleteView };
}
