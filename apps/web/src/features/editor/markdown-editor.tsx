import { Columns, Eye, Pencil } from "lucide-react";
import { useState } from "react";

import { MarkdownRenderer } from "@/components/markdown-renderer";
import { SizeIndicator } from "@/features/chunks/size-indicator";

type EditorMode = "edit" | "preview" | "split";

interface MarkdownEditorProps {
    value: string;
    onChange: (value: string) => void;
    placeholder?: string;
    rows?: number;
    error?: string;
}

export function MarkdownEditor({ value, onChange, placeholder, rows = 10, error }: MarkdownEditorProps) {
    const [mode, setMode] = useState<EditorMode>("edit");

    return (
        <div>
            <div className="mb-1.5 flex items-center justify-between">
                <label className="block text-sm font-medium">Content</label>
                <div className="flex items-center gap-2">
                    <SizeIndicator length={value.length} />
                    <div className="flex items-center gap-0.5 rounded-md border p-0.5">
                        <button
                            type="button"
                            onClick={() => setMode("edit")}
                            className={`flex items-center gap-1 rounded px-2 py-1 text-xs font-medium transition-colors ${
                                mode === "edit"
                                    ? "bg-accent text-accent-foreground"
                                    : "text-muted-foreground hover:text-foreground"
                            }`}
                        >
                            <Pencil className="size-3" />
                            Edit
                        </button>
                        <button
                            type="button"
                            onClick={() => setMode("preview")}
                            className={`flex items-center gap-1 rounded px-2 py-1 text-xs font-medium transition-colors ${
                                mode === "preview"
                                    ? "bg-accent text-accent-foreground"
                                    : "text-muted-foreground hover:text-foreground"
                            }`}
                        >
                            <Eye className="size-3" />
                            Preview
                        </button>
                        <button
                            type="button"
                            onClick={() => setMode("split")}
                            className={`flex items-center gap-1 rounded px-2 py-1 text-xs font-medium transition-colors ${
                                mode === "split"
                                    ? "bg-accent text-accent-foreground"
                                    : "text-muted-foreground hover:text-foreground"
                            }`}
                        >
                            <Columns className="size-3" />
                            Split
                        </button>
                    </div>
                </div>
            </div>
            <div className={mode === "split" ? "grid grid-cols-2 gap-4" : ""}>
                {mode !== "preview" && (
                    <textarea
                        value={value}
                        onChange={e => onChange(e.target.value)}
                        placeholder={placeholder}
                        rows={rows}
                        className="bg-background focus:ring-ring w-full resize-y rounded-md border px-3 py-2 text-sm focus:ring-2 focus:outline-none"
                    />
                )}
                {mode !== "edit" && (
                    <div className="max-h-[500px] min-h-[120px] overflow-y-auto rounded-md border px-3 py-2">
                        {!value.trim() ? (
                            <p className="text-muted-foreground text-sm italic">Nothing to preview</p>
                        ) : (
                            <div className="prose prose-sm dark:prose-invert max-w-none">
                                <MarkdownRenderer>{value}</MarkdownRenderer>
                            </div>
                        )}
                    </div>
                )}
            </div>
            {error && <p className="text-destructive mt-1 text-xs">{error}</p>}
        </div>
    );
}
