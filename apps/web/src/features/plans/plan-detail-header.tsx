import { useMutation } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { Archive, Check, Copy, Link2, Printer, Trash2 } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";

import { ConfirmDialog } from "@/components/confirm-dialog";
import { Button } from "@/components/ui/button";
import {
    Select,
    SelectItem,
    SelectPopup,
    SelectTrigger,
    SelectValue,
} from "@/components/ui/select";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

import { PlanStatusPill, type PlanStatusValue } from "./plan-status-pill";

const ALL_STATUSES: PlanStatusValue[] = ["draft", "analyzing", "ready", "in_progress", "completed", "archived"];

const STATUS_LABEL: Record<PlanStatusValue, string> = {
    draft: "Draft",
    analyzing: "Analyzing",
    ready: "Ready",
    in_progress: "In Progress",
    completed: "Completed",
    archived: "Archived",
};

export interface PlanDetailHeaderProps {
    plan: { id: string; title: string; status: PlanStatusValue; updatedAt: string };
    taskCount: { done: number; total: number };
    onUpdate: () => void;
}

export function PlanDetailHeader({ plan, taskCount, onUpdate }: PlanDetailHeaderProps) {
    const navigate = useNavigate();
    const [titleDraft, setTitleDraft] = useState(plan.title);
    const [editingTitle, setEditingTitle] = useState(false);
    const [confirmDelete, setConfirmDelete] = useState(false);
    const [copiedUrl, setCopiedUrl] = useState(false);

    const updateMutation = useMutation({
        mutationFn: async (patch: Record<string, unknown>) =>
            unwrapEden(await (api.api as any).plans[plan.id].patch(patch)),
        onSuccess: () => onUpdate(),
        onError: () => toast.error("Failed to update plan"),
    });

    const duplicateMutation = useMutation({
        mutationFn: async () =>
            unwrapEden(await (api.api as any).plans[plan.id].duplicate.post()) as { id: string; title: string },
        onSuccess: created => {
            toast.success(`Duplicated as "${created.title}"`);
            navigate({ to: "/plans/$planId", params: { planId: created.id } });
        },
        onError: () => toast.error("Failed to duplicate plan"),
    });

    const deleteMutation = useMutation({
        mutationFn: async () => unwrapEden(await (api.api as any).plans[plan.id].delete()),
        onSuccess: () => {
            toast.success("Plan deleted");
            navigate({ to: "/plans" });
        },
        onError: () => toast.error("Failed to delete plan"),
    });

    const onCopyUrl = () => {
        navigator.clipboard.writeText(window.location.href);
        setCopiedUrl(true);
        setTimeout(() => setCopiedUrl(false), 1500);
    };

    const progressPct = taskCount.total === 0 ? 0 : Math.round((taskCount.done / taskCount.total) * 100);

    return (
        <div className="sticky top-0 z-20 -mx-4 border-b bg-background/95 px-4 py-3 backdrop-blur">
            <div className="flex items-start gap-3">
                <div className="flex-1">
                    {editingTitle ? (
                        <input
                            autoFocus
                            value={titleDraft}
                            onChange={e => setTitleDraft(e.target.value)}
                            onBlur={() => {
                                setEditingTitle(false);
                                if (titleDraft !== plan.title) updateMutation.mutate({ title: titleDraft });
                            }}
                            onKeyDown={e => {
                                if (e.key === "Enter") e.currentTarget.blur();
                                if (e.key === "Escape") {
                                    setTitleDraft(plan.title);
                                    setEditingTitle(false);
                                }
                            }}
                            className="w-full bg-transparent text-xl font-semibold outline-none"
                        />
                    ) : (
                        <h1
                            className="cursor-text text-xl font-semibold hover:bg-muted/30 rounded px-1 -mx-1"
                            onClick={() => setEditingTitle(true)}
                            title="Click to rename"
                        >
                            {plan.title}
                        </h1>
                    )}
                    <div className="mt-1 flex items-center gap-2 text-xs text-muted-foreground">
                        <Select value={plan.status} onValueChange={val => { if (val) updateMutation.mutate({ status: val }); }}>
                            <SelectTrigger
                                size="sm"
                                className="h-auto w-auto min-h-0 border-0 bg-transparent p-0 shadow-none hover:opacity-80"
                            >
                                <SelectValue>
                                    <PlanStatusPill status={plan.status} />
                                </SelectValue>
                            </SelectTrigger>
                            <SelectPopup>
                                {ALL_STATUSES.map(s => (
                                    <SelectItem key={s} value={s}>{STATUS_LABEL[s]}</SelectItem>
                                ))}
                            </SelectPopup>
                        </Select>
                        <span>•</span>
                        <span className="font-mono">{taskCount.done}/{taskCount.total} tasks</span>
                        <div className="h-1 max-w-[120px] flex-1 overflow-hidden rounded bg-muted">
                            <div className="h-full bg-emerald-500" style={{ width: `${progressPct}%` }} />
                        </div>
                        <span>•</span>
                        <span>Updated {new Date(plan.updatedAt).toLocaleDateString()}</span>
                    </div>
                </div>
                <div className="flex gap-1">
                    <Button variant="ghost" size="sm" title="Copy link" onClick={onCopyUrl}>
                        {copiedUrl ? <Check className="size-4 text-emerald-500" /> : <Link2 className="size-4" />}
                    </Button>
                    <Button variant="ghost" size="sm" title="Print / export" onClick={() => window.print()}>
                        <Printer className="size-4" />
                    </Button>
                    <Button
                        variant="ghost"
                        size="sm"
                        title={plan.status === "archived" ? "Unarchive" : "Archive"}
                        onClick={() => updateMutation.mutate({ status: plan.status === "archived" ? "draft" : "archived" })}
                    >
                        <Archive className="size-4" />
                    </Button>
                    <Button
                        variant="ghost"
                        size="sm"
                        title="Duplicate"
                        onClick={() => duplicateMutation.mutate()}
                        disabled={duplicateMutation.isPending}
                    >
                        <Copy className="size-4" />
                    </Button>
                    <Button
                        variant="ghost"
                        size="sm"
                        title="Delete"
                        onClick={() => setConfirmDelete(true)}
                    >
                        <Trash2 className="size-4" />
                    </Button>
                </div>
            </div>

            <ConfirmDialog
                open={confirmDelete}
                onOpenChange={open => { if (!open) setConfirmDelete(false); }}
                title="Delete plan"
                description={`Delete plan "${plan.title}"? All linked tasks, analyze items, and requirement links will be removed. This cannot be undone.`}
                confirmLabel="Delete"
                confirmVariant="destructive"
                onConfirm={() => deleteMutation.mutate()}
                loading={deleteMutation.isPending}
            />
        </div>
    );
}
