import { cn } from "@/lib/utils";

export type PlanStatusValue =
    | "draft"
    | "analyzing"
    | "ready"
    | "in_progress"
    | "completed"
    | "archived";

const STATUS_STYLES: Record<PlanStatusValue, string> = {
    draft: "bg-slate-500/15 border-slate-500/30 text-slate-400",
    analyzing: "bg-blue-500/15 border-blue-500/30 text-blue-400",
    ready: "bg-indigo-500/15 border-indigo-500/30 text-indigo-400",
    in_progress: "bg-amber-500/15 border-amber-500/30 text-amber-400",
    completed: "bg-emerald-500/15 border-emerald-500/30 text-emerald-400",
    archived: "bg-zinc-500/15 border-zinc-500/30 text-zinc-500",
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
        <span
            className={cn(
                "inline-flex items-center rounded-full border px-2 py-0.5 text-[10px] font-semibold uppercase tracking-wide",
                STATUS_STYLES[status],
                className,
            )}
        >
            {STATUS_LABELS[status]}
        </span>
    );
}
