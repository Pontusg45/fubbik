import { Bookmark, Save, Trash2, X } from "lucide-react";
import { useState } from "react";

import {
    DropdownMenu,
    DropdownMenuContent,
    DropdownMenuItem,
    DropdownMenuLabel,
    DropdownMenuSeparator,
    DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { useLocalStorage } from "@/hooks/use-local-storage";

export interface FilterPresetFilters {
    activeTypes: string[];
    activeRelations: string[];
    activeTagTypeIds: string[];
}

interface FilterPreset {
    name: string;
    filters: FilterPresetFilters;
}

interface FilterPresetsProps {
    currentFilters: FilterPresetFilters;
    onApplyPreset: (filters: FilterPresetFilters) => void;
}

export function FilterPresets({ currentFilters, onApplyPreset }: FilterPresetsProps) {
    const [presets, setPresets] = useLocalStorage<FilterPreset[]>("fubbik-graph-filter-presets", []);
    const [isSaving, setIsSaving] = useState(false);
    const [presetName, setPresetName] = useState("");

    function handleSave() {
        const trimmed = presetName.trim();
        if (!trimmed) return;
        setPresets(prev => [...prev, { name: trimmed, filters: currentFilters }]);
        setPresetName("");
        setIsSaving(false);
    }

    function handleDelete(index: number, e: React.MouseEvent) {
        e.stopPropagation();
        e.preventDefault();
        setPresets(prev => prev.filter((_, i) => i !== index));
    }

    return (
        <div className="border-t pt-3">
            <div className="flex items-center gap-1">
                <DropdownMenu>
                    <DropdownMenuTrigger
                        className="text-muted-foreground hover:text-foreground flex cursor-pointer items-center gap-1 text-[10px] font-medium uppercase transition-colors"
                    >
                        <Bookmark className="size-3" />
                        Presets
                    </DropdownMenuTrigger>
                    <DropdownMenuContent side="bottom" align="start" sideOffset={8}>
                        <DropdownMenuLabel>Saved Presets</DropdownMenuLabel>
                        <DropdownMenuSeparator />
                        {presets.length === 0 ? (
                            <div className="text-muted-foreground px-2 py-3 text-center text-xs">
                                No saved presets yet
                            </div>
                        ) : (
                            presets.map((preset, i) => (
                                <DropdownMenuItem
                                    key={`${preset.name}-${i}`}
                                    className="flex items-center justify-between gap-2"
                                    onSelect={() => onApplyPreset(preset.filters)}
                                >
                                    <span className="truncate">{preset.name}</span>
                                    <button
                                        type="button"
                                        className="text-muted-foreground hover:text-destructive shrink-0 rounded p-0.5 transition-colors"
                                        onClick={(e) => handleDelete(i, e)}
                                        aria-label={`Delete preset ${preset.name}`}
                                    >
                                        <Trash2 className="size-3" />
                                    </button>
                                </DropdownMenuItem>
                            ))
                        )}
                    </DropdownMenuContent>
                </DropdownMenu>
            </div>

            {isSaving ? (
                <div className="mt-1.5 flex items-center gap-1">
                    <input
                        type="text"
                        value={presetName}
                        onChange={e => setPresetName(e.target.value)}
                        onKeyDown={e => {
                            if (e.key === "Enter") handleSave();
                            if (e.key === "Escape") {
                                setIsSaving(false);
                                setPresetName("");
                            }
                        }}
                        placeholder="Preset name..."
                        className="bg-background border-input min-w-0 flex-1 rounded border px-1.5 py-0.5 text-[10px] outline-none focus:ring-1 focus:ring-ring"
                        autoFocus
                    />
                    <button
                        type="button"
                        onClick={handleSave}
                        disabled={!presetName.trim()}
                        className="text-muted-foreground hover:text-foreground disabled:opacity-40"
                        aria-label="Confirm save"
                    >
                        <Save className="size-3" />
                    </button>
                    <button
                        type="button"
                        onClick={() => { setIsSaving(false); setPresetName(""); }}
                        className="text-muted-foreground hover:text-foreground"
                        aria-label="Cancel save"
                    >
                        <X className="size-3" />
                    </button>
                </div>
            ) : (
                <button
                    type="button"
                    onClick={() => setIsSaving(true)}
                    className="text-muted-foreground hover:text-foreground mt-1.5 flex cursor-pointer items-center gap-1 text-[10px] transition-colors"
                >
                    <Save className="size-3" />
                    Save current filters
                </button>
            )}
        </div>
    );
}
