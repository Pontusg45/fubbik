import { useMutation } from "@tanstack/react-query";

import { InlineEdit } from "@/components/ui/inline-edit";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

export interface PlanDescriptionSectionProps {
    planId: string;
    description: string | null;
    onUpdate: () => void;
}

export function PlanDescriptionSection({ planId, description, onUpdate }: PlanDescriptionSectionProps) {
    const updateMutation = useMutation({
        mutationFn: async (body: Record<string, unknown>) =>
            unwrapEden(await (api.api as any).plans[planId].patch(body)),
        onSuccess: () => onUpdate(),
    });

    return (
        <section className="space-y-2">
            <h2 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">Description</h2>
            <InlineEdit
                as="textarea"
                rows={8}
                value={description ?? ""}
                onSave={next => updateMutation.mutate({ description: next || null })}
                placeholder="Describe what this plan is about"
                renderDisplay={val => <div className="whitespace-pre-wrap text-sm">{val}</div>}
                className="block w-full"
            />
        </section>
    );
}
