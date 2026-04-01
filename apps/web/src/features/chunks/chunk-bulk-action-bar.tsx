import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Archive, Edit, Link2, Search, Tags, Trash2, X } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { Separator } from "@/components/ui/separator";
import { BulkTagEditor } from "@/features/chunks/bulk-tag-editor";
import { api } from "@/utils/api";

interface ChunkBulkActionBarProps {
    selectedIds: Set<string>;
    setSelectedIds: (ids: Set<string>) => void;
    bulkUpdateMutation: {
        mutate: (body: { ids: string[]; action: string; value?: string | null }) => void;
        isPending: boolean;
    };
    setConfirmAction: (action: { title: string; description: string; action: () => void } | null) => void;
}

export function ChunkBulkActionBar({
    selectedIds,
    setSelectedIds,
    bulkUpdateMutation,
    setConfirmAction,
}: ChunkBulkActionBarProps) {
    const queryClient = useQueryClient();

    // Bulk tag editor dialog
    const [showBulkTagEditor, setShowBulkTagEditor] = useState(false);

    // Tag input state
    const [showBulkTagInput, setShowBulkTagInput] = useState(false);
    const [bulkTagInput, setBulkTagInput] = useState("");
    const [bulkTagAction, setBulkTagAction] = useState<"add_tags" | "remove_tags">("add_tags");

    // Connect state
    const [showBulkConnect, setShowBulkConnect] = useState(false);
    const [connectSearch, setConnectSearch] = useState("");
    const [connectRelation, setConnectRelation] = useState("related_to");

    const connectSearchQuery = useQuery({
        queryKey: ["chunks", "bulk-connect-search", connectSearch],
        queryFn: async () => {
            if (!connectSearch.trim()) return { chunks: [] };
            const { data, error } = await api.api.chunks.get({ query: { search: connectSearch, limit: "10" } });
            if (error) throw new Error("Failed to search chunks");
            return data;
        },
        enabled: showBulkConnect && connectSearch.trim().length > 0
    });

    const bulkConnectMutation = useMutation({
        mutationFn: async (targetId: string) => {
            const ids = [...selectedIds];
            for (const sourceId of ids) {
                const { error } = await api.api.connections.post({
                    sourceId,
                    targetId,
                    relation: connectRelation
                });
                if (error) throw new Error("Failed to create connection");
            }
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["chunks-list"] });
            toast.success(`Connected ${selectedIds.size} chunks`);
            setShowBulkConnect(false);
            setConnectSearch("");
            setConnectRelation("related_to");
            setSelectedIds(new Set());
        },
        onError: () => {
            toast.error("Failed to create connections");
        }
    });

    const connectResults = (connectSearchQuery.data?.chunks ?? []).filter(c => !selectedIds.has(c.id));

    function handleBulkDelete() {
        setConfirmAction({
            title: `Delete ${selectedIds.size} chunk${selectedIds.size !== 1 ? "s" : ""}?`,
            description: "This action cannot be undone. All selected chunks will be permanently deleted.",
            action: () => bulkUpdateMutation.mutate({ ids: [...selectedIds], action: "delete" }),
        });
    }

    function handleBulkArchive() {
        setConfirmAction({
            title: `Archive ${selectedIds.size} chunk${selectedIds.size !== 1 ? "s" : ""}?`,
            description: "Archived chunks can be restored later from the archive view.",
            action: () => bulkUpdateMutation.mutate({ ids: [...selectedIds], action: "archive" }),
        });
    }

    function handleBulkTagSubmit() {
        if (!bulkTagInput.trim()) return;
        bulkUpdateMutation.mutate({
            ids: [...selectedIds],
            action: bulkTagAction,
            value: bulkTagInput.trim()
        });
        setBulkTagInput("");
        setShowBulkTagInput(false);
    }

    if (selectedIds.size === 0) return null;

    return (
        <div className="bg-background fixed bottom-6 left-1/2 z-50 flex -translate-x-1/2 items-center gap-3 rounded-lg border px-4 py-2.5 shadow-lg">
            <span className="text-sm font-medium">{selectedIds.size} selected</span>
            <Separator orientation="vertical" className="h-5" />
            {showBulkTagInput ? (
                <div className="flex items-center gap-1.5">
                    <input
                        type="text"
                        value={bulkTagInput}
                        onChange={e => setBulkTagInput(e.target.value)}
                        onKeyDown={e => e.key === "Enter" && handleBulkTagSubmit()}
                        placeholder="tag1, tag2, ..."
                        className="bg-background w-36 rounded border px-2 py-1 text-xs"
                        autoFocus
                    />
                    <Button size="sm" variant="outline" onClick={handleBulkTagSubmit} disabled={bulkUpdateMutation.isPending}>
                        {bulkTagAction === "add_tags" ? "Add" : "Remove"}
                    </Button>
                    <Button size="sm" variant="ghost" onClick={() => setShowBulkTagInput(false)}>
                        <X className="size-3" />
                    </Button>
                </div>
            ) : (
                <>
                    <Button
                        variant="outline"
                        size="sm"
                        onClick={() => {
                            setBulkTagAction("add_tags");
                            setShowBulkTagInput(true);
                        }}
                    >
                        <Tags className="size-3.5" />
                        Add Tags
                    </Button>
                    <Button
                        variant="outline"
                        size="sm"
                        onClick={() => {
                            setBulkTagAction("remove_tags");
                            setShowBulkTagInput(true);
                        }}
                    >
                        <Tags className="size-3.5" />
                        Remove Tags
                    </Button>
                    <Button
                        variant="outline"
                        size="sm"
                        onClick={() => setShowBulkTagEditor(true)}
                    >
                        <Edit className="size-3.5" />
                        Edit Tags
                    </Button>
                </>
            )}
            <Popover open={showBulkConnect} onOpenChange={setShowBulkConnect}>
                <PopoverTrigger render={<Button variant="outline" size="sm" />}>
                    <Link2 className="size-3.5" />
                    Connect to...
                </PopoverTrigger>
                <PopoverContent side="top" align="center" className="w-80">
                    <div className="space-y-3">
                        <div>
                            <label className="text-muted-foreground mb-1.5 block text-xs font-medium">Relation</label>
                            <select
                                value={connectRelation}
                                onChange={e => setConnectRelation(e.target.value)}
                                className="bg-background w-full rounded-md border px-2.5 py-1.5 text-sm"
                            >
                                {["related_to", "part_of", "depends_on", "extends", "references", "supports", "contradicts", "alternative_to"].map(r => (
                                    <option key={r} value={r}>{r.replace(/_/g, " ")}</option>
                                ))}
                            </select>
                        </div>
                        <div>
                            <label className="text-muted-foreground mb-1.5 block text-xs font-medium">Target chunk</label>
                            <div className="relative">
                                <Search className="text-muted-foreground pointer-events-none absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2" />
                                <input
                                    type="text"
                                    value={connectSearch}
                                    onChange={e => setConnectSearch(e.target.value)}
                                    placeholder="Search chunks..."
                                    className="bg-background w-full rounded-md border py-1.5 pl-8 pr-3 text-sm"
                                    autoFocus
                                />
                            </div>
                        </div>
                        {connectResults.length > 0 && (
                            <div className="max-h-48 space-y-0.5 overflow-y-auto rounded-md border p-1">
                                {connectResults.map(result => (
                                    <button
                                        key={result.id}
                                        type="button"
                                        onClick={() => bulkConnectMutation.mutate(result.id)}
                                        disabled={bulkConnectMutation.isPending}
                                        className="hover:bg-muted flex w-full items-center justify-between rounded-md px-2.5 py-1.5 text-left text-sm transition-colors"
                                    >
                                        <span className="truncate font-medium">{result.title}</span>
                                        <Badge variant="secondary" size="sm" className="ml-2 shrink-0 text-[10px]">
                                            {result.type}
                                        </Badge>
                                    </button>
                                ))}
                            </div>
                        )}
                        {connectSearch.trim().length > 0 && connectResults.length === 0 && !connectSearchQuery.isLoading && (
                            <p className="text-muted-foreground py-2 text-center text-xs">No chunks found</p>
                        )}
                    </div>
                </PopoverContent>
            </Popover>
            <select
                onChange={e => {
                    if (e.target.value) {
                        bulkUpdateMutation.mutate({ ids: [...selectedIds], action: "set_type", value: e.target.value });
                    }
                }}
                className="bg-background rounded-md border px-2 py-1 text-xs"
                defaultValue=""
            >
                <option value="" disabled>
                    Set Type...
                </option>
                {["note", "decision", "pattern", "convention", "rule", "reference"].map(t => (
                    <option key={t} value={t}>
                        {t}
                    </option>
                ))}
            </select>
            <select
                onChange={e => {
                    if (e.target.value) {
                        bulkUpdateMutation.mutate({
                            ids: [...selectedIds],
                            action: "set_review_status",
                            value: e.target.value
                        });
                    }
                }}
                className="bg-background rounded-md border px-2 py-1 text-xs"
                defaultValue=""
            >
                <option value="" disabled>
                    Set Status...
                </option>
                <option value="draft">Draft</option>
                <option value="reviewed">Reviewed</option>
                <option value="approved">Approved</option>
            </select>
            <Separator orientation="vertical" className="h-5" />
            <Button variant="outline" size="sm" onClick={handleBulkArchive} disabled={bulkUpdateMutation.isPending}>
                <Archive className="size-3.5" />
                Archive
            </Button>
            <Button variant="destructive" size="sm" onClick={handleBulkDelete} disabled={bulkUpdateMutation.isPending}>
                <Trash2 className="size-3.5" />
                Delete
            </Button>
            <Button variant="ghost" size="sm" onClick={() => setSelectedIds(new Set())}>
                Cancel
            </Button>
            <BulkTagEditor
                chunkIds={[...selectedIds]}
                open={showBulkTagEditor}
                onOpenChange={setShowBulkTagEditor}
            />
        </div>
    );
}
