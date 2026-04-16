import { Filter } from "lucide-react";
import { useMemo, useState } from "react";

import { Button } from "@/components/ui/button";
import { Dialog, DialogHeader, DialogPopup, DialogTitle } from "@/components/ui/dialog";
import { applyPrefilter } from "@/features/graph/apply-prefilter";
import {
    EMPTY_FILTER,
    GraphFilterForm,
    type GraphFilterPreviewData,
    type GraphFilterValues
} from "@/features/graph/graph-filter-form";

export type { GraphFilterValues };
export { EMPTY_FILTER };

interface GraphFilterDialogProps {
    open: boolean;
    onOpenChange: (open: boolean) => void;
    initial: GraphFilterValues;
    onApply: (values: GraphFilterValues) => void;
    onShowEverything: () => void;
    previewData?: GraphFilterPreviewData;
}

/**
 * Modal wrapper around GraphFilterForm. State is buffered here — edits only
 * commit via Apply. The inline top-left panel uses the same form component but
 * commits on every change.
 */
export function GraphFilterDialog({
    open,
    onOpenChange,
    initial,
    onApply,
    onShowEverything,
    previewData
}: GraphFilterDialogProps) {
    const [draft, setDraft] = useState<GraphFilterValues>(initial);

    // Re-sync when `initial` changes across dialog openings.
    const [syncedFor, setSyncedFor] = useState<GraphFilterValues | null>(null);
    if (open && syncedFor !== initial) {
        setDraft(initial);
        setSyncedFor(initial);
    }

    function handleApply() {
        onApply(draft);
        onOpenChange(false);
    }

    function handleShowEverything() {
        onShowEverything();
        onOpenChange(false);
    }

    // Don't allow Apply when the current draft would render zero chunks.
    const disableApply = useMemo(() => {
        if (!previewData) return false;
        const { chunks } = applyPrefilter(previewData, draft);
        return chunks.length === 0;
    }, [previewData, draft]);

    return (
        <Dialog open={open} onOpenChange={onOpenChange}>
            <DialogPopup className="max-w-xl">
                <DialogHeader>
                    <DialogTitle>
                        <span className="inline-flex items-center gap-2">
                            <Filter className="size-4" />
                            Filter the graph
                        </span>
                    </DialogTitle>
                    <p className="text-muted-foreground text-sm">
                        Choose what to show before we render. You can change filters later from the top-left panel.
                    </p>
                </DialogHeader>

                <div className="space-y-5 px-6 pb-6">
                    <GraphFilterForm values={draft} onChange={setDraft} previewData={previewData} />

                    <div className="flex items-center justify-end gap-2 border-t pt-4">
                        <Button variant="ghost" size="sm" onClick={handleShowEverything}>
                            Show everything
                        </Button>
                        <Button size="sm" onClick={handleApply} disabled={disableApply}>
                            Apply filter
                        </Button>
                    </div>
                </div>
            </DialogPopup>
        </Dialog>
    );
}
