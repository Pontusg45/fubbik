import { StatusPill, type StatusPillVariant } from "@/components/ui/status-pill";

export type PlanStatusValue =
    | "draft"
    | "analyzing"
    | "ready"
    | "in_progress"
    | "completed"
    | "archived";

const STATUS_VARIANT: Record<PlanStatusValue, StatusPillVariant> = {
    draft: "slate",
    analyzing: "blue",
    ready: "indigo",
    in_progress: "amber",
    completed: "emerald",
    archived: "zinc",
};

const STATUS_LABELS: Record<PlanStatusValue, string> = {
    draft: "Draft",
    analyzing: "Analyzing",
    ready: "Ready",
    in_progress: "In Progress",
    completed: "Completed",
    archived: "Archived",
};

export function PlanStatusPill({ status, className }: { status: PlanStatusValue; className?: string }) {
    return (
        <StatusPill
            variant={STATUS_VARIANT[status]}
            label={STATUS_LABELS[status]}
            className={className}
        />
    );
}
