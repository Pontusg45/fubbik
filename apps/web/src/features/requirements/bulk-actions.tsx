import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useRef, useState } from "react";
import { toast } from "sonner";

import { ConfirmDialog } from "@/components/confirm-dialog";
import { Button } from "@/components/ui/button";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

interface BulkActionsProps {
    selectedIds: string[];
    onClearSelection: () => void;
    useCases: Array<{ id: string; name: string }>;
}

export function BulkActions({ selectedIds, onClearSelection, useCases }: BulkActionsProps) {
    const queryClient = useQueryClient();

    const [showDeleteConfirm, setShowDeleteConfirm] = useState(false);
    const pendingCountRef = useRef(selectedIds.length);

    const bulkMutation = useMutation({
        mutationFn: async (body: {
            ids: string[];
            action: "set_status" | "set_use_case" | "delete";
            status?: "passing" | "failing" | "untested";
            useCaseId?: string | null;
        }) => {
            pendingCountRef.current = body.ids.length;
            return unwrapEden(await api.api.requirements.bulk.patch(body));
        },
        onSuccess: (_data, variables) => {
            queryClient.invalidateQueries({ queryKey: ["requirements"] });
            queryClient.invalidateQueries({ queryKey: ["requirements-stats"] });
            const count = pendingCountRef.current;
            switch (variables.action) {
                case "set_status":
                    toast.success(`Set ${count} requirement(s) to ${variables.status}`);
                    break;
                case "set_use_case":
                    toast.success(variables.useCaseId ? "Assigned use case" : "Removed use case assignment");
                    break;
                case "delete":
                    toast.success(`Deleted ${count} requirement(s)`);
                    break;
            }
            onClearSelection();
        },
        onError: () => {
            toast.error("Bulk operation failed");
        }
    });

    function handleSetStatus(status: "passing" | "failing" | "untested") {
        bulkMutation.mutate({ ids: selectedIds, action: "set_status", status });
    }

    function handleAssignUseCase(useCaseId: string | null) {
        bulkMutation.mutate({ ids: selectedIds, action: "set_use_case", useCaseId });
    }

    function handleDelete() {
        setShowDeleteConfirm(false);
        bulkMutation.mutate({ ids: selectedIds, action: "delete" });
    }

    if (selectedIds.length === 0) return null;

    return (
        <div className="fixed bottom-6 left-1/2 z-50 flex -translate-x-1/2 items-center gap-3 rounded-xl border bg-card px-5 py-3 shadow-lg">
            <span className="text-sm font-medium">
                {selectedIds.length} selected
            </span>

            <div className="bg-border h-5 w-px" />

            {/* Set Status */}
            <div className="relative">
                <select
                    className="bg-background cursor-pointer appearance-none rounded-md border px-3 py-1.5 pr-7 text-sm"
                    defaultValue=""
                    onChange={e => {
                        if (e.target.value) {
                            handleSetStatus(e.target.value as "passing" | "failing" | "untested");
                            e.target.value = "";
                        }
                    }}
                    disabled={bulkMutation.isPending}
                >
                    <option value="" disabled>Set Status</option>
                    <option value="passing">Passing</option>
                    <option value="failing">Failing</option>
                    <option value="untested">Untested</option>
                </select>
            </div>

            {/* Assign Use Case */}
            {useCases.length > 0 && (
                <div className="relative">
                    <select
                        className="bg-background cursor-pointer appearance-none rounded-md border px-3 py-1.5 pr-7 text-sm"
                        defaultValue=""
                        onChange={e => {
                            if (e.target.value === "__none__") {
                                handleAssignUseCase(null);
                            } else if (e.target.value) {
                                handleAssignUseCase(e.target.value);
                            }
                            e.target.value = "";
                        }}
                        disabled={bulkMutation.isPending}
                    >
                        <option value="" disabled>Assign Use Case</option>
                        <option value="__none__">None (unassign)</option>
                        {useCases.map(uc => (
                            <option key={uc.id} value={uc.id}>{uc.name}</option>
                        ))}
                    </select>
                </div>
            )}

            <Button
                variant="destructive"
                size="sm"
                onClick={() => setShowDeleteConfirm(true)}
                disabled={bulkMutation.isPending}
            >
                Delete
            </Button>
            <ConfirmDialog
                open={showDeleteConfirm}
                onOpenChange={setShowDeleteConfirm}
                title="Delete requirements"
                description={`Delete ${selectedIds.length} requirement(s)? This cannot be undone.`}
                confirmLabel="Delete"
                confirmVariant="destructive"
                onConfirm={handleDelete}
                loading={bulkMutation.isPending}
            />

            <Button
                variant="ghost"
                size="sm"
                onClick={onClearSelection}
            >
                Clear
            </Button>
        </div>
    );
}
