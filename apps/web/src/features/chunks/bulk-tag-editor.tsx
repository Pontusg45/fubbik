import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Plus, X } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { toast } from "sonner";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
    Dialog,
    DialogPopup,
    DialogHeader,
    DialogTitle,
    DialogDescription,
    DialogPanel,
    DialogFooter,
} from "@/components/ui/dialog";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

interface BulkTagEditorProps {
    chunkIds: string[];
    open: boolean;
    onOpenChange: (open: boolean) => void;
}

interface ChunkTagState {
    id: string;
    title: string;
    tags: string[];
    originalTags: string[];
}

export function BulkTagEditor({ chunkIds, open, onOpenChange }: BulkTagEditorProps) {
    const queryClient = useQueryClient();
    const [chunkStates, setChunkStates] = useState<ChunkTagState[]>([]);
    const [addingTagForChunk, setAddingTagForChunk] = useState<string | null>(null);
    const [tagInput, setTagInput] = useState("");
    const [filteredSuggestions, setFilteredSuggestions] = useState<string[]>([]);
    const inputRef = useRef<HTMLInputElement>(null);

    // Fetch all available tags for autocomplete
    const tagsQuery = useQuery({
        queryKey: ["tags-all"],
        queryFn: async () => unwrapEden(await api.api.tags.get()) as Array<{ id: string; name: string }>,
        enabled: open,
    });

    const allTagNames = useMemo(() => (tagsQuery.data ?? []).map(t => t.name), [tagsQuery.data]);

    // Fetch chunk details for all selected IDs
    const chunksQuery = useQuery({
        queryKey: ["bulk-tag-chunks", chunkIds],
        queryFn: async () => {
            const results = await Promise.all(
                chunkIds.map(async id => {
                    try {
                        const data = unwrapEden(await api.api.chunks({ id }).get());
                        return data;
                    } catch {
                        return null;
                    }
                })
            );
            return results.filter((c): c is NonNullable<typeof c> => !!c);
        },
        enabled: open && chunkIds.length > 0,
    });

    // Initialize chunk states when data loads
    useEffect(() => {
        if (chunksQuery.data) {
            setChunkStates(
                chunksQuery.data.map(c => {
                    const chunk = (c as any).chunk ?? c;
                    const tags = ((chunk.tags ?? []) as Array<{ id: string; name: string }>).map(t => t.name);
                    return {
                        id: chunk.id,
                        title: chunk.title,
                        tags: [...tags],
                        originalTags: [...tags],
                    };
                })
            );
        }
    }, [chunksQuery.data]);

    const removeTag = useCallback((chunkId: string, tagName: string) => {
        setChunkStates(prev =>
            prev.map(cs =>
                cs.id === chunkId ? { ...cs, tags: cs.tags.filter(t => t !== tagName) } : cs
            )
        );
    }, []);

    const addTag = useCallback((chunkId: string, tagName: string) => {
        const trimmed = tagName.trim().toLowerCase();
        if (!trimmed) return;
        setChunkStates(prev =>
            prev.map(cs =>
                cs.id === chunkId && !cs.tags.includes(trimmed)
                    ? { ...cs, tags: [...cs.tags, trimmed] }
                    : cs
            )
        );
        setTagInput("");
        setAddingTagForChunk(null);
    }, []);

    const handleInputChange = useCallback(
        (value: string) => {
            setTagInput(value);
            if (value.trim()) {
                const lower = value.toLowerCase();
                setFilteredSuggestions(allTagNames.filter(t => t.toLowerCase().includes(lower)).slice(0, 5));
            } else {
                setFilteredSuggestions([]);
            }
        },
        [allTagNames]
    );

    const saveMutation = useMutation({
        mutationFn: async () => {
            const changed = chunkStates.filter(cs => {
                const orig = cs.originalTags.slice().sort().join(",");
                const curr = cs.tags.slice().sort().join(",");
                return orig !== curr;
            });
            for (const cs of changed) {
                await api.api.chunks({ id: cs.id }).patch({ tags: cs.tags });
            }
            return changed.length;
        },
        onSuccess: (count) => {
            toast.success(`Updated tags on ${count} chunk${count !== 1 ? "s" : ""}`);
            queryClient.invalidateQueries({ queryKey: ["chunks-list"] });
            queryClient.invalidateQueries({ queryKey: ["tags"] });
            onOpenChange(false);
        },
        onError: () => {
            toast.error("Failed to update tags");
        },
    });

    const hasChanges = chunkStates.some(cs => {
        const orig = cs.originalTags.slice().sort().join(",");
        const curr = cs.tags.slice().sort().join(",");
        return orig !== curr;
    });

    return (
        <Dialog open={open} onOpenChange={onOpenChange}>
            <DialogPopup className="max-w-2xl">
                <DialogHeader>
                    <DialogTitle>Edit Tags</DialogTitle>
                    <DialogDescription>
                        Manage tags for {chunkIds.length} selected chunk{chunkIds.length !== 1 ? "s" : ""}.
                    </DialogDescription>
                </DialogHeader>
                <DialogPanel>
                    {chunksQuery.isLoading ? (
                        <div className="text-muted-foreground py-8 text-center text-sm">Loading chunks...</div>
                    ) : (
                        <div className="divide-border divide-y">
                            {chunkStates.map(cs => (
                                <div key={cs.id} className="flex items-start gap-3 py-3">
                                    <div className="min-w-0 flex-1">
                                        <p className="truncate text-sm font-medium">{cs.title}</p>
                                        <div className="mt-1.5 flex flex-wrap items-center gap-1">
                                            {cs.tags.map(tag => (
                                                <Badge key={tag} variant="secondary" size="sm" className="gap-1 pr-1 text-[11px]">
                                                    {tag}
                                                    <button
                                                        type="button"
                                                        onClick={() => removeTag(cs.id, tag)}
                                                        className="text-muted-foreground hover:text-foreground ml-0.5 rounded transition-colors"
                                                    >
                                                        <X className="size-2.5" />
                                                    </button>
                                                </Badge>
                                            ))}
                                            {addingTagForChunk === cs.id ? (
                                                <div className="relative">
                                                    <input
                                                        ref={inputRef}
                                                        type="text"
                                                        value={tagInput}
                                                        onChange={e => handleInputChange(e.target.value)}
                                                        onKeyDown={e => {
                                                            if (e.key === "Enter" && tagInput.trim()) {
                                                                addTag(cs.id, tagInput);
                                                            }
                                                            if (e.key === "Escape") {
                                                                setAddingTagForChunk(null);
                                                                setTagInput("");
                                                            }
                                                        }}
                                                        onBlur={() => {
                                                            // Delay to allow click on suggestion
                                                            setTimeout(() => {
                                                                setAddingTagForChunk(null);
                                                                setTagInput("");
                                                                setFilteredSuggestions([]);
                                                            }, 150);
                                                        }}
                                                        placeholder="Tag name..."
                                                        className="bg-background h-5 w-24 rounded border px-1.5 text-[11px]"
                                                        autoFocus
                                                    />
                                                    {filteredSuggestions.length > 0 && (
                                                        <div className="bg-popover absolute top-full left-0 z-10 mt-0.5 w-36 rounded-md border p-0.5 shadow-md">
                                                            {filteredSuggestions.map(s => (
                                                                <button
                                                                    key={s}
                                                                    type="button"
                                                                    className="hover:bg-accent flex w-full rounded-sm px-2 py-1 text-left text-[11px] transition-colors"
                                                                    onMouseDown={e => {
                                                                        e.preventDefault();
                                                                        addTag(cs.id, s);
                                                                    }}
                                                                >
                                                                    {s}
                                                                </button>
                                                            ))}
                                                        </div>
                                                    )}
                                                </div>
                                            ) : (
                                                <button
                                                    type="button"
                                                    onClick={() => {
                                                        setAddingTagForChunk(cs.id);
                                                        setTagInput("");
                                                    }}
                                                    className="text-muted-foreground hover:text-foreground hover:bg-muted flex size-5 items-center justify-center rounded transition-colors"
                                                >
                                                    <Plus className="size-3" />
                                                </button>
                                            )}
                                        </div>
                                    </div>
                                </div>
                            ))}
                        </div>
                    )}
                </DialogPanel>
                <DialogFooter>
                    <Button variant="outline" size="sm" onClick={() => onOpenChange(false)}>
                        Cancel
                    </Button>
                    <Button
                        size="sm"
                        onClick={() => saveMutation.mutate()}
                        disabled={!hasChanges || saveMutation.isPending}
                    >
                        {saveMutation.isPending ? "Saving..." : "Save Changes"}
                    </Button>
                </DialogFooter>
            </DialogPopup>
        </Dialog>
    );
}
