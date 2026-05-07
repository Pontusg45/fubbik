"use client";

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
        <div className="flex flex-col gap-4 p-4 overflow-y-auto h-full">
            {/* File path breadcrumb */}
            <div className="text-xs text-muted-foreground truncate">
                {pathParts.join(" / ")}
            </div>

            {/* Title heading */}
            <h3 className="font-semibold text-base leading-tight">{preview.parsed.title}</h3>

            {/* Title field */}
            <div className="flex flex-col gap-1.5">
                <label className="text-xs font-medium text-muted-foreground uppercase tracking-wide">
                    Title
                </label>
                <input
                    type="text"
                    className="border-input bg-background block w-full rounded-md border px-3 py-1.5 text-sm focus:outline-none focus:ring-2 focus:ring-ring"
                    value={config.title}
                    onChange={e => update({ title: e.target.value })}
                />
            </div>

            {/* Type field */}
            <div className="flex flex-col gap-1.5">
                <label className="text-xs font-medium text-muted-foreground uppercase tracking-wide">
                    Type
                </label>
                <select
                    className="border-input bg-background block w-full rounded-md border px-3 py-1.5 text-sm focus:outline-none focus:ring-2 focus:ring-ring"
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
                <label className="text-xs font-medium text-muted-foreground uppercase tracking-wide">
                    Tags
                </label>
                <div className="flex flex-wrap gap-1.5 rounded-md border border-input bg-background px-2 py-1.5 min-h-9">
                    {config.tags.map(tag => {
                        const isFolderTag = config.folderTags.includes(tag);
                        return (
                            <Badge
                                key={tag}
                                variant={isFolderTag ? "outline" : "secondary"}
                                className={isFolderTag ? "border-dashed" : undefined}
                            >
                                {tag}
                                <button
                                    type="button"
                                    onClick={() => handleRemoveTag(tag)}
                                    className="ml-0.5 hover:opacity-70"
                                >
                                    <X className="size-3" />
                                </button>
                            </Badge>
                        );
                    })}
                    <input
                        ref={tagInputRef}
                        type="text"
                        className="min-w-16 flex-1 bg-transparent text-sm outline-none placeholder:text-muted-foreground"
                        placeholder="Add tag..."
                        value={tagInput}
                        onChange={e => setTagInput(e.target.value)}
                        onKeyDown={handleAddTag}
                    />
                </div>
            </div>

            {/* Template section */}
            <div className="flex flex-col gap-1.5">
                <label className="text-xs font-medium text-muted-foreground uppercase tracking-wide">
                    Template
                </label>

                {suggestedTemplate ? (
                    <div className="rounded-md border border-emerald-500/20 bg-emerald-500/5 p-3 flex flex-col gap-2">
                        <div className="flex items-center gap-2">
                            <Check className="size-4 text-emerald-600 shrink-0" />
                            <span className="text-sm font-medium">{suggestedTemplate.name}</span>
                            <Badge variant="success" size="sm" className="ml-auto shrink-0">
                                {Math.round(suggestedTemplate.score * 100)}%
                            </Badge>
                        </div>

                        {/* Extracted fields */}
                        {Object.keys(suggestedTemplate.extractedFields).length > 0 && (
                            <div className="flex flex-col gap-0.5 text-xs text-muted-foreground">
                                {Object.entries(suggestedTemplate.extractedFields).map(([k, v]) => (
                                    <div key={k} className="flex gap-1.5">
                                        <span className="font-medium shrink-0">{k}:</span>
                                        <span className="truncate">{String(v)}</span>
                                    </div>
                                ))}
                            </div>
                        )}

                        {/* Override dropdown */}
                        <div className="flex items-center gap-2">
                            <span className="text-xs text-muted-foreground">Override:</span>
                            <select
                                className="border-input bg-background flex-1 rounded border px-2 py-0.5 text-xs focus:outline-none focus:ring-1 focus:ring-ring"
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
                                <span className="text-xs text-muted-foreground">
                                    {siblingCount - 1} other file{siblingCount - 1 !== 1 ? "s" : ""} in this folder —
                                </span>
                                <Button
                                    variant="outline"
                                    size="sm"
                                    className="h-5 px-2 text-xs"
                                    onClick={() =>
                                        onApplyToFolder(activeTemplateId ?? suggestedTemplate.id)
                                    }
                                >
                                    Apply to folder
                                </Button>
                            </div>
                        )}
                    </div>
                ) : (
                    <div className="flex flex-col gap-1.5">
                        <p className="text-xs text-muted-foreground">No template matched. Choose manually:</p>
                        <select
                            className="border-input bg-background block w-full rounded-md border px-3 py-1.5 text-sm focus:outline-none focus:ring-2 focus:ring-ring"
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
                <label className="text-xs font-medium text-muted-foreground uppercase tracking-wide">
                    Content preview
                </label>
                <div className="rounded-md border border-input bg-muted/30 px-3 py-2 text-xs text-muted-foreground overflow-y-auto max-h-32 whitespace-pre-wrap font-mono">
                    {preview.parsed.content.slice(0, 500)}
                    {preview.parsed.content.length > 500 && (
                        <span className="opacity-50">…</span>
                    )}
                </div>
            </div>
        </div>
    );
}
