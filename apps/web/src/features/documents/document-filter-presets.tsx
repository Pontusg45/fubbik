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

export interface DocPresetFilters {
    activeTags: string[];
    activeTypes: string[];
    groupBy: "folder" | "tag";
}

interface DocFilterPreset {
    name: string;
    filters: DocPresetFilters;
}

interface DocFilterPresetsProps {
    currentFilters: DocPresetFilters;
    onApplyPreset: (filters: DocPresetFilters) => void;
}

export function DocFilterPresets({ currentFilters, onApplyPreset }: DocFilterPresetsProps) {
    const [presets, setPresets] = useLocalStorage<DocFilterPreset[]>("fubbik-docs-filter-presets", []);
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
        <div className="flex items-center gap-2 text-[11px]">
            <DropdownMenu>
                <DropdownMenuTrigger className="text-muted-foreground hover:text-foreground flex cursor-pointer items-center gap-1 transition-colors">
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

            {isSaving ? (
                <div className="flex items-center gap-1">
                    <input
                        type="text"
                        value={presetName}
                        onChange={e => setPresetName(e.target.value)}
                        onKeyDown={e => {
                            if (e.key === "Enter") handleSave();
                            if (e.key === "Escape") { setIsSaving(false); setPresetName(""); }
                        }}
                        placeholder="Name..."
                        className="bg-background border-input min-w-0 flex-1 rounded border px-1.5 py-0.5 text-[10px] outline-none focus:ring-1 focus:ring-ring"
                        autoFocus
                    />
                    <button type="button" onClick={handleSave} disabled={!presetName.trim()} className="text-muted-foreground hover:text-foreground disabled:opacity-40">
                        <Save className="size-3" />
                    </button>
                    <button type="button" onClick={() => { setIsSaving(false); setPresetName(""); }} className="text-muted-foreground hover:text-foreground">
                        <X className="size-3" />
                    </button>
                </div>
            ) : (
                <button
                    type="button"
                    onClick={() => setIsSaving(true)}
                    className="text-muted-foreground hover:text-foreground flex cursor-pointer items-center gap-1 transition-colors"
                >
                    <Save className="size-3" />
                    Save
                </button>
            )}
        </div>
    );
}
