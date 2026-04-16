import { Download, ExternalLink, FileCode2, Save, Settings2 } from "lucide-react";

import { Popover, PopoverTrigger, PopoverPopup } from "@/components/ui/popover";
import type { LayoutAlgorithm } from "@/features/graph/layouts";

interface SavedView {
    name: string;
    filterTypes: string[];
    filterRelations: string[];
    collapsedParents: string[];
    layoutAlgorithm: string;
    focusNodeId?: string;
}

interface SavedGraph {
    id: string;
    name: string;
}

export interface GraphSettingsPanelProps {
    layoutAlgorithm: LayoutAlgorithm;
    onLayoutChange: (algorithm: LayoutAlgorithm) => void;
    hasDraggedPositions: boolean;
    onResetLayout: () => void;
    exploreMode: boolean;
    exploredNodeIds: Set<string>;
    onToggleExploreMode: () => void;
    onResetExplored: () => void;
    bundleEdges: boolean;
    onToggleBundleEdges: () => void;
    useMainThread: boolean;
    onToggleMainThread: () => void;
    onSaveView: () => void;
    savedViews: SavedView[];
    onRestoreView: (view: SavedView) => void;
    onDeleteView: (name: string) => void;
    onSaveCustomGraph: () => void;
    savedGraphs: SavedGraph[];
    onOpenGraph: (id: string) => void;
    onDeleteGraph: (id: string) => void;
    onExportImage: () => void;
    onExportMermaid: () => void;
}

export function GraphSettingsPanel({
    layoutAlgorithm,
    onLayoutChange,
    hasDraggedPositions,
    onResetLayout,
    exploreMode,
    exploredNodeIds,
    onToggleExploreMode,
    onResetExplored,
    bundleEdges,
    onToggleBundleEdges,
    useMainThread,
    onToggleMainThread,
    onSaveView,
    savedViews,
    onRestoreView,
    onDeleteView,
    onSaveCustomGraph,
    savedGraphs,
    onOpenGraph,
    onDeleteGraph,
    onExportImage,
    onExportMermaid,
}: GraphSettingsPanelProps) {
    return (
        <Popover>
            <PopoverTrigger className="bg-background/80 text-muted-foreground hover:text-foreground rounded-md border p-1.5 backdrop-blur-sm">
                <Settings2 className="size-4" />
            </PopoverTrigger>
            <PopoverPopup side="bottom" align="end" sideOffset={8} className="w-56">
                <div className="flex flex-col gap-3">
                    <div>
                        <label className="text-muted-foreground mb-1 block text-[10px] font-medium tracking-wider uppercase">
                            Layout
                        </label>
                        <select
                            value={layoutAlgorithm}
                            onChange={e => onLayoutChange(e.target.value as LayoutAlgorithm)}
                            className="bg-background w-full rounded-md border px-2 py-1.5 text-xs"
                        >
                            <option value="force">Force-directed</option>
                            <option value="hierarchical">Tree</option>
                            <option value="radial">Radial</option>
                        </select>
                    </div>
                    {hasDraggedPositions && (
                        <button
                            onClick={onResetLayout}
                            className="text-muted-foreground hover:text-foreground w-full rounded-md border px-2.5 py-1.5 text-left text-xs"
                        >
                            Reset layout
                        </button>
                    )}
                    <div className="flex flex-col gap-2">
                        <label className="text-muted-foreground text-[10px] font-medium tracking-wider uppercase">Tools</label>
                        <button
                            onClick={onToggleExploreMode}
                            className={`w-full rounded-md border px-2.5 py-1.5 text-left text-xs ${
                                exploreMode
                                    ? "bg-primary text-primary-foreground"
                                    : "text-muted-foreground hover:text-foreground"
                            }`}
                        >
                            Explore mode
                            {exploreMode && <span className="ml-1 opacity-70">({exploredNodeIds.size})</span>}
                        </button>
                        {exploreMode && (
                            <button
                                onClick={onResetExplored}
                                className="text-muted-foreground hover:text-foreground w-full rounded-md border px-2.5 py-1.5 text-left text-xs"
                            >
                                Reset explored
                            </button>
                        )}
                        <button
                            onClick={onToggleBundleEdges}
                            className={`w-full rounded-md border px-2.5 py-1.5 text-left text-xs ${
                                bundleEdges
                                    ? "bg-primary text-primary-foreground"
                                    : "text-muted-foreground hover:text-foreground"
                            }`}
                        >
                            Bundle edges
                        </button>
                        <button
                            onClick={onToggleMainThread}
                            className={`w-full rounded-md border px-2.5 py-1.5 text-left text-xs ${
                                useMainThread
                                    ? "bg-primary text-primary-foreground"
                                    : "text-muted-foreground hover:text-foreground"
                            }`}
                        >
                            Main-thread layout
                        </button>
                    </div>
                    <div className="flex flex-col gap-2">
                        <label className="text-muted-foreground text-[10px] font-medium tracking-wider uppercase">Views</label>
                        <button
                            onClick={onSaveView}
                            className="text-muted-foreground hover:text-foreground w-full rounded-md border px-2.5 py-1.5 text-left text-xs"
                        >
                            Save current view...
                        </button>
                        {savedViews.map(view => (
                            <div key={view.name} className="flex items-center justify-between gap-1">
                                <button
                                    className="text-muted-foreground hover:text-foreground flex-1 truncate rounded-md border px-2.5 py-1.5 text-left text-xs"
                                    onClick={() => onRestoreView(view)}
                                >
                                    {view.name}
                                </button>
                                <button
                                    className="text-muted-foreground hover:text-destructive shrink-0 rounded p-1"
                                    onClick={() => onDeleteView(view.name)}
                                >
                                    <span className="text-[10px]">&#x2715;</span>
                                </button>
                            </div>
                        ))}
                    </div>
                    <div className="flex flex-col gap-2">
                        <label className="text-muted-foreground text-[10px] font-medium tracking-wider uppercase">Custom Graphs</label>
                        <button
                            onClick={onSaveCustomGraph}
                            className="text-muted-foreground hover:text-foreground flex w-full items-center gap-2 rounded-md border px-2.5 py-1.5 text-left text-xs"
                        >
                            <Save className="size-3" />
                            Save as custom graph...
                        </button>
                        {savedGraphs.map(sg => (
                            <div key={sg.id} className="flex items-center justify-between gap-1">
                                <button
                                    className="text-muted-foreground hover:text-foreground flex flex-1 items-center gap-1.5 truncate rounded-md border px-2.5 py-1.5 text-left text-xs"
                                    onClick={() => onOpenGraph(sg.id)}
                                >
                                    <ExternalLink className="size-3 shrink-0" />
                                    {sg.name}
                                </button>
                                <button
                                    className="text-muted-foreground hover:text-destructive shrink-0 rounded p-1"
                                    onClick={() => onDeleteGraph(sg.id)}
                                >
                                    <span className="text-[10px]">&#x2715;</span>
                                </button>
                            </div>
                        ))}
                    </div>
                    <div className="flex flex-col gap-2 border-t pt-3">
                        <button
                            onClick={onExportImage}
                            className="text-muted-foreground hover:text-foreground flex w-full items-center gap-2 rounded-md border px-2.5 py-1.5 text-xs"
                        >
                            <Download className="size-3.5" />
                            Export as PNG
                        </button>
                        <button
                            onClick={onExportMermaid}
                            className="text-muted-foreground hover:text-foreground flex w-full items-center gap-2 rounded-md border px-2.5 py-1.5 text-xs"
                        >
                            <FileCode2 className="size-3.5" />
                            Export as Mermaid
                        </button>
                    </div>
                </div>
            </PopoverPopup>
        </Popover>
    );
}
