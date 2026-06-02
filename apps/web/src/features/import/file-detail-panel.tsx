import { Check, X } from "lucide-react";
import { useRef, useState } from "react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";

import type { FileConfig, PreviewFileResult } from "./types";

// ---------------------------------------------------------------------------
// Props
// ---------------------------------------------------------------------------

interface FileDetailPanelProps {
    filePath: string;
    preview: PreviewFileResult;
    config: FileConfig;
    onConfigChange: (path: string, config: FileConfig) => void;
    templates: { id: string; name: string }[];
    siblingCount?: number;
    onApplyToFolder?: (templateId: string) => void;
}

// ---------------------------------------------------------------------------
// FileDetailPanel
// ---------------------------------------------------------------------------

export function FileDetailPanel({
    filePath,
    preview,
    config,
    onConfigChange,
    templates,
    siblingCount,
    onApplyToFolder
}: FileDetailPanelProps) {
    const [tagInput, setTagInput] = useState("");
    const tagInputRef = useRef<HTMLInputElement>(null);

    const pathParts = filePath.split("/");

    const update = (partial: Partial<FileConfig>) => {
        onConfigChange(filePath, { ...config, ...partial });
    };

    const handleRemoveTag = (tag: string) => {
        update({ tags: config.tags.filter(t => t !== tag) });
    };

    const handleAddTag = (e: React.KeyboardEvent<HTMLInputElement>) => {
        if (e.key === "Enter") {
            e.preventDefault();
            const newTag = tagInput.trim();
            if (newTag && !config.tags.includes(newTag)) {
                update({ tags: [...config.tags, newTag] });
            }
            setTagInput("");
        }
    };

    const handleTemplateOverride = (templateId: string) => {
        update({ templateId: templateId || null });
    };

    const suggestedTemplate = preview.suggestedTemplate;
    const activeTemplateId = config.templateId;

    return (
        <div className="flex h-full flex-col gap-4 overflow-y-auto p-4">
            {/* File path breadcrumb */}
            <div className="text-muted-foreground truncate text-xs">{pathParts.join(" / ")}</div>

            {/* Title heading */}
            <h3 className="text-base leading-tight font-semibold">{preview.parsed.title}</h3>

            {/* Title field */}
            <div className="flex flex-col gap-1.5">
                <label className="text-muted-foreground text-xs font-medium tracking-wide uppercase">Title</label>
                <input
                    type="text"
                    className="border-input bg-background focus:ring-ring block w-full rounded-md border px-3 py-1.5 text-sm focus:ring-2 focus:outline-none"
                    value={config.title}
                    onChange={e => update({ title: e.target.value })}
                />
            </div>

            {/* Type field */}
            <div className="flex flex-col gap-1.5">
                <label className="text-muted-foreground text-xs font-medium tracking-wide uppercase">Type</label>
                <select
                    className="border-input bg-background focus:ring-ring block w-full rounded-md border px-3 py-1.5 text-sm focus:ring-2 focus:outline-none"
                    value={config.type}
                    onChange={e => update({ type: e.target.value })}
                >
                    <option value="document">document</option>
                    <option value="note">note</option>
                    <option value="reference">reference</option>
                    <option value="schema">schema</option>
                    <option value="checklist">checklist</option>
                </select>
            </div>

            {/* Tags field */}
            <div className="flex flex-col gap-1.5">
                <label className="text-muted-foreground text-xs font-medium tracking-wide uppercase">Tags</label>
                <div className="border-input bg-background flex min-h-9 flex-wrap gap-1.5 rounded-md border px-2 py-1.5">
                    {config.tags.map(tag => {
                        const isFolderTag = config.folderTags.includes(tag);
                        return (
                            <Badge
                                key={tag}
                                variant={isFolderTag ? "outline" : "secondary"}
                                className={isFolderTag ? "border-dashed" : undefined}
                            >
                                {tag}
                                <button type="button" onClick={() => handleRemoveTag(tag)} className="ml-0.5 hover:opacity-70">
                                    <X className="size-3" />
                                </button>
                            </Badge>
                        );
                    })}
                    <input
                        ref={tagInputRef}
                        type="text"
                        className="placeholder:text-muted-foreground min-w-16 flex-1 bg-transparent text-sm outline-none"
                        placeholder="Add tag..."
                        value={tagInput}
                        onChange={e => setTagInput(e.target.value)}
                        onKeyDown={handleAddTag}
                    />
                </div>
            </div>

            {/* Template section */}
            <div className="flex flex-col gap-1.5">
                <label className="text-muted-foreground text-xs font-medium tracking-wide uppercase">Template</label>

                {suggestedTemplate ? (
                    <div className="flex flex-col gap-2 rounded-md border border-emerald-500/20 bg-emerald-500/5 p-3">
                        <div className="flex items-center gap-2">
                            <Check className="size-4 shrink-0 text-emerald-600" />
                            <span className="text-sm font-medium">{suggestedTemplate.name}</span>
                            <Badge variant="success" size="sm" className="ml-auto shrink-0">
                                {Math.round(suggestedTemplate.score * 100)}%
                            </Badge>
                        </div>

                        {/* Extracted fields */}
                        {Object.keys(suggestedTemplate.extractedFields).length > 0 && (
                            <div className="text-muted-foreground flex flex-col gap-0.5 text-xs">
                                {Object.entries(suggestedTemplate.extractedFields).map(([k, v]) => (
                                    <div key={k} className="flex gap-1.5">
                                        <span className="shrink-0 font-medium">{k}:</span>
                                        <span className="truncate">{String(v)}</span>
                                    </div>
                                ))}
                            </div>
                        )}

                        {/* Override dropdown */}
                        <div className="flex items-center gap-2">
                            <span className="text-muted-foreground text-xs">Override:</span>
                            <select
                                className="border-input bg-background focus:ring-ring flex-1 rounded border px-2 py-0.5 text-xs focus:ring-1 focus:outline-none"
                                value={activeTemplateId ?? suggestedTemplate.id}
                                onChange={e => handleTemplateOverride(e.target.value)}
                            >
                                <option value={suggestedTemplate.id}>{suggestedTemplate.name} (suggested)</option>
                                <option value="">None</option>
                                {templates
                                    .filter(t => t.id !== suggestedTemplate.id)
                                    .map(t => (
                                        <option key={t.id} value={t.id}>
                                            {t.name}
                                        </option>
                                    ))}
                            </select>
                        </div>

                        {/* Apply to folder hint */}
                        {onApplyToFolder && siblingCount != null && siblingCount > 1 && (
                            <div className="flex items-center gap-2 pt-0.5">
                                <span className="text-muted-foreground text-xs">
                                    {siblingCount - 1} other file{siblingCount - 1 !== 1 ? "s" : ""} in this folder —
                                </span>
                                <Button
                                    variant="outline"
                                    size="sm"
                                    className="h-5 px-2 text-xs"
                                    onClick={() => onApplyToFolder(activeTemplateId ?? suggestedTemplate.id)}
                                >
                                    Apply to folder
                                </Button>
                            </div>
                        )}
                    </div>
                ) : (
                    <div className="flex flex-col gap-1.5">
                        <p className="text-muted-foreground text-xs">No template matched. Choose manually:</p>
                        <select
                            className="border-input bg-background focus:ring-ring block w-full rounded-md border px-3 py-1.5 text-sm focus:ring-2 focus:outline-none"
                            value={activeTemplateId ?? ""}
                            onChange={e => handleTemplateOverride(e.target.value)}
                        >
                            <option value="">No template</option>
                            {templates.map(t => (
                                <option key={t.id} value={t.id}>
                                    {t.name}
                                </option>
                            ))}
                        </select>
                    </div>
                )}
            </div>

            {/* Content preview */}
            <div className="flex flex-col gap-1.5">
                <label className="text-muted-foreground text-xs font-medium tracking-wide uppercase">Content preview</label>
                <div className="border-input bg-muted/30 text-muted-foreground max-h-32 overflow-y-auto rounded-md border px-3 py-2 font-mono text-xs whitespace-pre-wrap">
                    {preview.parsed.content.slice(0, 500)}
                    {preview.parsed.content.length > 500 && <span className="opacity-50">…</span>}
                </div>
            </div>
        </div>
    );
}
