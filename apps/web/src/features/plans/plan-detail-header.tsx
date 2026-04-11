import { useMutation } from "@tanstack/react-query";
import { Archive, Copy, Trash2 } from "lucide-react";
import { useState } from "react";

import { Button } from "@/components/ui/button";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

import { PlanStatusPill, type PlanStatusValue } from "./plan-status-pill";

const STATUS_CYCLE: PlanStatusValue[] = ["draft", "analyzing", "ready", "in_progress", "completed"];

export interface PlanDetailHeaderProps {
    plan: { id: string; title: string; status: PlanStatusValue; updatedAt: string };
    taskCount: { done: number; total: number };
    onUpdate: () => void;
}

export function PlanDetailHeader({ plan, taskCount, onUpdate }: PlanDetailHeaderProps) {
    const [titleDraft, setTitleDraft] = useState(plan.title);
    const [editingTitle, setEditingTitle] = useState(false);

    const updateMutation = useMutation({
        mutationFn: async (patch: Record<string, unknown>) =>
            unwrapEden(await (api.api as any).plans[plan.id].patch(patch)),
        onSuccess: () => onUpdate(),
    });

    const cycleStatus = () => {
        const idx = STATUS_CYCLE.indexOf(plan.status);
        const next = STATUS_CYCLE[(idx + 1) % STATUS_CYCLE.length];
        updateMutation.mutate({ status: next });
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
                            className="cursor-text text-xl font-semibold"
                            onClick={() => setEditingTitle(true)}
                        >
                            {plan.title}
                        </h1>
                    )}
                    <div className="mt-1 flex items-center gap-2 text-xs text-muted-foreground">
                        <button type="button" onClick={cycleStatus} className="hover:opacity-80">
                            <PlanStatusPill status={plan.status} />
                        </button>
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
                    <Button variant="ghost" size="sm" title="Archive" onClick={() => updateMutation.mutate({ status: "archived" })}>
                        <Archive className="size-4" />
                    </Button>
                    <Button variant="ghost" size="sm" title="Duplicate">
                        <Copy className="size-4" />
                    </Button>
                    <Button variant="ghost" size="sm" title="Delete">
                        <Trash2 className="size-4" />
                    </Button>
                </div>
            </div>
        </div>
    );
}
