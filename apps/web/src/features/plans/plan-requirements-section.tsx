import { useMutation, useQuery } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { ChevronRight, Plus, X } from "lucide-react";
import { useState } from "react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export interface PlanRequirementsSectionProps {
    planId: string;
    requirements: Array<{ id: string; requirementId: string; order: number }>;
    onUpdate: () => void;
}

export function PlanRequirementsSection({ planId, requirements, onUpdate }: PlanRequirementsSectionProps) {
    const [pickerOpen, setPickerOpen] = useState(false);
    const [pickerQuery, setPickerQuery] = useState("");

    const reqIds = requirements.map(r => r.requirementId);

    const requirementDetailsQuery = useQuery({
        queryKey: ["plan-requirements-detail", reqIds],
        queryFn: async () => {
            if (reqIds.length === 0) return [];
            const res = unwrapEden(await api.api.requirements.get({ query: {} as any })) as any;
            const all: any[] = res?.requirements ?? res ?? [];
            return all.filter((r: any) => reqIds.includes(r.id));
        },
    });

    const searchQuery = useQuery({
        queryKey: ["requirements-search", pickerQuery],
        queryFn: async () => {
            const res = unwrapEden(await api.api.requirements.get({ query: {} as any })) as any;
            const all: any[] = res?.requirements ?? res ?? [];
            if (!pickerQuery) return all.slice(0, 10);
            return all
                .filter((r: any) => r.title.toLowerCase().includes(pickerQuery.toLowerCase()))
                .slice(0, 10);
        },
        enabled: pickerOpen,
    });

    const addMutation = useMutation({
        mutationFn: async (requirementId: string) =>
            unwrapEden(await (api.api as any).plans[planId].requirements.post({ requirementId })),
        onSuccess: () => {
            setPickerOpen(false);
            setPickerQuery("");
            onUpdate();
        },
    });

    const removeMutation = useMutation({
        mutationFn: async (requirementId: string) =>
            unwrapEden(await (api.api as any).plans[planId].requirements[requirementId].delete()),
        onSuccess: () => onUpdate(),
    });

    const linkedIds = new Set(reqIds);
    const available = (searchQuery.data ?? []).filter((r: any) => !linkedIds.has(r.id));

    return (
        <section className="space-y-2">
            <div className="flex items-center justify-between">
                <h2 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                    Requirements <span className="ml-1 font-mono text-muted-foreground/60">({requirements.length})</span>
                </h2>
                <Button size="sm" variant="ghost" onClick={() => setPickerOpen(o => !o)}>
                    <Plus className="size-3.5" />
                    Add
                </Button>
            </div>
            {pickerOpen && (
                <div className="rounded-md border bg-card p-2 shadow-sm">
                    <Input
                        autoFocus
                        placeholder="Search requirements…"
                        value={pickerQuery}
                        onChange={e => setPickerQuery(e.target.value)}
                        className="mb-2 h-8 text-sm"
                    />
                    <div className="max-h-60 space-y-1 overflow-y-auto">
                        {available.length === 0 ? (
                            <div className="py-3 text-center text-xs text-muted-foreground">No matches</div>
                        ) : (
                            available.map((r: any) => (
                                <button
                                    key={r.id}
                                    type="button"
                                    onClick={() => addMutation.mutate(r.id)}
                                    className="flex w-full items-center gap-2 rounded px-2 py-1.5 text-left text-xs hover:bg-muted"
                                >
                                    <span className="flex-1 truncate">{r.title}</span>
                                    <span className="text-[9px] uppercase text-muted-foreground">{r.priority}</span>
                                </button>
                            ))
                        )}
                    </div>
                </div>
            )}
            {requirements.length === 0 ? (
                <div className="rounded-md border border-dashed p-4 text-center text-xs text-muted-foreground">
                    No requirements linked. Add one to document what this plan must satisfy.
                </div>
            ) : (
                <div className="space-y-1">
                    {(requirementDetailsQuery.data ?? []).map((r: any) => (
                        <div key={r.id} className="flex items-center gap-2 rounded-md border px-3 py-2">
                            <span className="flex-1 truncate text-sm">{r.title}</span>
                            <span className="text-[10px] uppercase text-muted-foreground">{r.status}</span>
                            <span className="text-[10px] uppercase text-muted-foreground">{r.priority}</span>
                            <button
                                type="button"
                                onClick={() => removeMutation.mutate(r.id)}
                                className="text-muted-foreground hover:text-foreground"
                                aria-label="Remove requirement"
                            >
                                <X className="size-3" />
                            </button>
                            <Link to="/requirements/$requirementId" params={{ requirementId: r.id }} className="text-muted-foreground hover:text-foreground">
                                <ChevronRight className="size-4" />
                            </Link>
                        </div>
                    ))}
                </div>
            )}
        </section>
    );
}
