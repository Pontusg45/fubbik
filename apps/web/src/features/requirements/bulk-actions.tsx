import { useMutation, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";

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

    const bulkMutation = useMutation({
        mutationFn: async (body: {
            ids: string[];
            action: "set_status" | "set_use_case" | "delete";
            status?: "passing" | "failing" | "untested";
            useCaseId?: string | null;
        }) => {
            return unwrapEden(await api.api.requirements.bulk.patch(body));
        },
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: ["requirements"] });
            queryClient.invalidateQueries({ queryKey: ["requirements-stats"] });
            onClearSelection();
        },
        onError: () => {
            toast.error("Bulk operation failed");
        }
    });

    function handleSetStatus(status: "passing" | "failing" | "untested") {
        bulkMutation.mutate({
            ids: selectedIds,
            action: "set_status",
            status
        });
        toast.success(`Set ${selectedIds.length} requirement(s) to ${status}`);
    }

    function handleAssignUseCase(useCaseId: string | null) {
        bulkMutation.mutate({
            ids: selectedIds,
            action: "set_use_case",
            useCaseId
        });
        toast.success(useCaseId ? "Assigned use case" : "Removed use case assignment");
    }

    function handleDelete() {
        if (!confirm(`Delete ${selectedIds.length} requirement(s)?`)) return;
        bulkMutation.mutate({
            ids: selectedIds,
            action: "delete"
        });
        toast.success(`Deleted ${selectedIds.length} requirement(s)`);
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
                onClick={handleDelete}
                disabled={bulkMutation.isPending}
            >
                Delete
            </Button>

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
