import { InlineEdit } from "@/components/ui/inline-edit";
import { useApiMutation } from "@/hooks/use-api-mutation";
import { api } from "@/utils/api";

export interface PlanDescriptionSectionProps {
    planId: string;
    description: string | null;
    onUpdate: () => void;
}

export function PlanDescriptionSection({ planId, description, onUpdate }: PlanDescriptionSectionProps) {
    const updateMutation = useApiMutation({
        mutationFn: async (body: Record<string, unknown>) => await api.api.plans({ id: planId }).patch(body),
        successToast: false,
        errorToast: "Failed to update description",
        onSuccess: () => onUpdate()
    });

    return (
        <section className="space-y-2">
            <h2 className="text-muted-foreground text-xs font-semibold tracking-wider uppercase">Description</h2>
            <InlineEdit
                as="textarea"
                rows={8}
                value={description ?? ""}
                onSave={next => updateMutation.mutate({ description: next || null })}
                placeholder="Describe what this plan is about"
                renderDisplay={val => <div className="text-sm whitespace-pre-wrap">{val}</div>}
                className="block w-full"
            />
        </section>
    );
}
