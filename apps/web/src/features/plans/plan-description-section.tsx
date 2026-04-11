import { useMutation } from "@tanstack/react-query";
import { useState } from "react";

import { Textarea } from "@/components/ui/textarea";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export interface PlanDescriptionSectionProps {
    planId: string;
    description: string | null;
    onUpdate: () => void;
}

export function PlanDescriptionSection({ planId, description, onUpdate }: PlanDescriptionSectionProps) {
    const [draft, setDraft] = useState(description ?? "");
    const [editing, setEditing] = useState(false);

    const updateMutation = useMutation({
        mutationFn: async (body: Record<string, unknown>) =>
            unwrapEden(await (api.api as any).plans[planId].patch(body)),
        onSuccess: () => {
            setEditing(false);
            onUpdate();
        },
    });

    const handleSave = () => {
        if (draft !== (description ?? "")) {
            updateMutation.mutate({ description: draft || null });
        } else {
            setEditing(false);
        }
    };

    return (
        <section className="space-y-2">
            <h2 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">Description</h2>
            {editing ? (
                <Textarea
                    autoFocus
                    value={draft}
                    onChange={e => setDraft(e.target.value)}
                    onBlur={handleSave}
                    rows={8}
                    placeholder="Describe what this plan is about"
                />
            ) : (
                <div
                    className="cursor-text rounded-md border border-transparent p-2 hover:border-border"
                    onClick={() => setEditing(true)}
                >
                    {description ? (
                        <div className="whitespace-pre-wrap text-sm">{description}</div>
                    ) : (
                        <div className="text-sm text-muted-foreground">Describe what this plan is about</div>
                    )}
                </div>
            )}
        </section>
    );
}
